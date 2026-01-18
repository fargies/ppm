/*
** Copyright (C) 2025 Sylvain Fargier
**
** This software is provided 'as-is', without any express or implied
** warranty.  In no event will the authors be held liable for any damages
** arising from the use of this software.
**
** Permission is granted to anyone to use this software for any purpose,
** including commercial applications, and to alter it and redistribute it
** freely, subject to the following restrictions:
**
** 1. The origin of this software must not be misrepresented; you must not
**    claim that you wrote the original software. If you use this software
**    in a product, an acknowledgment in the product documentation would be
**    appreciated but is not required.
** 2. Altered source versions must be plainly marked as such, and must not be
**    misrepresented as being the original software.
** 3. This notice may not be removed or altered from any source distribution.
**
** Author: Sylvain Fargier <fargier.sylvain@gmail.com>
*/

use serde::{
    Deserialize, Serialize, Serializer,
    de::{Error, Visitor},
    ser::{SerializeMap, SerializeSeq},
};
use std::{
    fmt,
    ops::Deref,
    path::{Path, PathBuf},
    sync::LazyLock,
};

use crate::utils::GlobSet;

static DEFAULT_EXCLUDE: LazyLock<GlobSet> =
    LazyLock::new(|| GlobSet::try_from([".*", "**/{build,target}*", "*.o"]).unwrap());
const DEFAULT_MAX_DEPTH: usize = 4;

/// Directory watching object
#[derive(PartialEq, Clone)]
pub struct Watch {
    /// excluded paths globbing patterns
    pub exclude: Option<GlobSet>,
    /// force-included paths globbing patterns
    pub include: Option<GlobSet>,
    /// Paths to watch
    pub paths: Vec<PathBuf>,
    /// Maximum depth
    pub max_depth: usize,
}

impl Default for Watch {
    fn default() -> Self {
        Self {
            exclude: Default::default(),
            include: Default::default(),
            paths: Default::default(),
            max_depth: DEFAULT_MAX_DEPTH,
        }
    }
}

impl Watch {
    #[inline]
    pub fn add(&mut self, path: &Path) {
        self.paths.push(path.to_path_buf());
    }

    pub fn is_excluded(&self, path: &Path) -> bool {
        !self.include.as_ref().map_or(false, |g| g.is_match(path))
            && (self.exclude.as_ref().map_or(false, |g| g.is_match(path))
                || DEFAULT_EXCLUDE.is_match(path))
    }
}

impl fmt::Debug for Watch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut f = f.debug_struct("Watch");
        if let Some(include) = self.include.as_ref() {
            f.field("include", include);
        }
        if let Some(exclude) = self.exclude.as_ref() {
            f.field("exclude", exclude);
        }
        if self.max_depth != DEFAULT_MAX_DEPTH {
            f.field("max_depth", &self.max_depth);
        }
        f.field("paths", &self.paths).finish()
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(untagged)]
enum OneOrMany<T> {
    /// Single value
    One(T),
    /// Array of values
    Vec(Vec<T>),
}

impl<T> Into<Vec<T>> for OneOrMany<T> {
    fn into(self) -> Vec<T> {
        match self {
            OneOrMany::One(e) => vec![e],
            OneOrMany::Vec(items) => items,
        }
    }
}

impl<T> Serialize for OneOrMany<&T>
where
    T: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            OneOrMany::One(v) => v.serialize(serializer),
            OneOrMany::Vec(items) => {
                let mut seq = serializer.serialize_seq(Some(items.len()))?;
                for i in items.iter() {
                    seq.serialize_element(i)?;
                }
                seq.end()
            }
        }
    }
}

struct WatchVisitor();

impl<'de> Visitor<'de> for WatchVisitor {
    type Value = Watch;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter
            .write_str("a watch object (ex: `{ \"include\": \"*\", \"paths\": [ \"/tmp\" ] })`")
    }

    fn visit_str<E>(self, v: &str) -> std::result::Result<Self::Value, E>
    where
        E: Error,
    {
        let mut watch = Watch::default();
        watch.add(&PathBuf::from(v));
        Ok(watch)
    }

    fn visit_seq<A>(self, mut seq: A) -> std::result::Result<Self::Value, A::Error>
    where
        A: serde::de::SeqAccess<'de>,
    {
        let mut watch = Watch::default();
        while let Some(value) = seq.next_element()? {
            watch.add(value);
        }
        Ok(watch)
    }

    fn visit_map<A>(self, mut map: A) -> std::result::Result<Self::Value, A::Error>
    where
        A: serde::de::MapAccess<'de>,
    {
        let mut watch = Watch::default();
        while let Some(k) = map.next_key::<String>()? {
            if k == "exclude" {
                watch.exclude = Some(map.next_value()?);
            } else if k == "include" {
                watch.include = Some(map.next_value()?);
            } else if k == "paths" {
                watch.paths = map.next_value::<OneOrMany<PathBuf>>()?.into();
            } else if k == "max_depth" {
                watch.max_depth = map.next_value()?;
            }
        }
        Ok(watch)
    }
}

impl<'de> Deserialize<'de> for Watch {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(WatchVisitor())
    }
}

trait OptionLen {
    fn len(&self) -> usize;
}

impl<T> OptionLen for Option<T> {
    fn len(&self) -> usize {
        if self.is_some() { 1 } else { 0 }
    }
}

struct OneOrManyWrapper<'a, V, T>(&'a V)
where
    V: Deref<Target = [T]>,
    T: Serialize;

impl<'a, V, T> Serialize for OneOrManyWrapper<'a, V, T>
where
    V: Deref<Target = [T]>,
    T: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if self.0.len() == 1 {
            self.0.first().unwrap().serialize(serializer)
        } else {
            self.0.serialize(serializer)
        }
    }
}

impl Serialize for Watch {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if self.include.is_none() && self.exclude.is_none() && self.max_depth == DEFAULT_MAX_DEPTH {
            OneOrManyWrapper(&self.paths).serialize(serializer)
        } else {
            let mut map =
                serializer.serialize_map(Some(1 + self.include.len() + self.exclude.len()))?;
            if let Some(include) = self.include.as_ref() {
                map.serialize_entry("include", &include)?;
            }
            if let Some(exclude) = self.exclude.as_ref() {
                map.serialize_entry("exclude", &exclude)?;
            }
            if self.max_depth != DEFAULT_MAX_DEPTH {
                map.serialize_entry("max_depth", &self.max_depth)?;
            }
            map.serialize_entry("paths", &OneOrManyWrapper(&self.paths))?;
            map.end()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::{Result, anyhow};
    use serde_yaml_ng as yaml;

    #[test]
    fn serde() -> Result<()> {
        let watch = yaml::from_str::<Watch>("/tmp")?;
        assert_eq!(None, watch.include);
        assert_eq!(None, watch.exclude);
        assert_eq!(vec![PathBuf::from("/tmp")], watch.paths);

        assert_eq!(watch, yaml::from_str::<Watch>("{ paths: /tmp }")?);

        let watch =
            yaml::from_str::<Watch>("{ include: \"*.c\", exclude: \"*.h\", paths: [ /tmp ] }")?;
        assert_eq!(Some(GlobSet::try_from(["*.c"])?), watch.include);
        assert_eq!(Some(GlobSet::try_from(["*.h"])?), watch.exclude);
        assert_eq!(vec![PathBuf::from("/tmp")], watch.paths);

        assert_eq!(
            watch,
            yaml::from_str::<Watch>(
                "{ include: [ \"*.c\" ], exclude: [ \"*.h\" ], paths: [ /tmp ] }"
            )?
        );

        let watch = yaml::from_str::<Watch>("{}")?;
        assert_eq!(None, watch.include);
        assert_eq!(None, watch.exclude);
        assert!(watch.paths.is_empty());
        Ok(())
    }

    #[test]
    fn invalid() -> Result<()> {
        for test in ["paths: null", "paths: 32", "include: 32"] {
            if yaml::from_str::<Watch>(test).is_ok() {
                Err(anyhow!("should have failed to parse `{test}`"))?;
            }
        }
        Ok(())
    }

    #[test]
    fn exclusion() -> Result<()> {
        let mut watch = Watch::default();
        assert!(
            watch.is_excluded(&Path::new(".hidden")),
            "files starting with `.` should be excluded"
        );
        assert!(!watch.is_excluded(&Path::new("visible")));
        assert!(
            watch.is_excluded(&Path::new("/some/path/build-x86")),
            "dirs starting with `build` should be excluded"
        );

        watch.exclude = Some(GlobSet::try_from(["*.k"])?);
        watch.include = Some(GlobSet::try_from(["toto.k"])?);
        assert!(watch.is_excluded(&Path::new("toto1.k")));
        assert!(!watch.is_excluded(&Path::new("toto.k")));
        Ok(())
    }
}
