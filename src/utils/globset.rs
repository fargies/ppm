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

use anyhow::Result;
use globset::{Candidate, Glob, GlobMatcher};
use serde::{
    Deserialize, Deserializer, Serialize,
    de::{Error, SeqAccess, Visitor},
    ser::SerializeSeq,
};
use std::{fmt::Debug, path::Path};

/// Set of globbing patterns
///
/// the [globset::GlobSet] does not serialize when this one does
#[derive(Clone, Default)]
pub struct GlobSet(Vec<GlobMatcher>);

impl GlobSet {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_match(&self, path: &Path) -> bool {
        let candidate = Candidate::new(path);
        self.0.iter().any(|g| g.is_match_candidate(&candidate))
    }

    pub fn add<T>(&mut self, value: T) -> Result<()>
    where
        T: AsRef<str>,
    {
        self.0.push(Glob::new(value.as_ref())?.compile_matcher());
        Ok(())
    }
}

impl<T> TryFrom<&[T]> for GlobSet
where
    T: AsRef<str>,
{
    type Error = anyhow::Error;

    fn try_from(value: &[T]) -> Result<Self, Self::Error> {
        let mut ret = GlobSet(Vec::with_capacity(value.len()));
        for value in value {
            ret.add(value)?;
        }
        Ok(ret)
    }
}

impl<T, const N: usize> TryFrom<[T; N]> for GlobSet
where
    T: AsRef<str>,
{
    type Error = anyhow::Error;

    fn try_from(value: [T; N]) -> Result<Self, Self::Error> {
        let mut ret = GlobSet(Vec::with_capacity(N));
        for value in value {
            ret.add(value)?;
        }
        Ok(ret)
    }
}

impl PartialEq for GlobSet {
    fn eq(&self, other: &Self) -> bool {
        self.0
            .iter()
            .zip(other.0.iter())
            .all(|(x, y)| x.glob() == y.glob())
    }
}

/// [OneOrMany] like serializer
impl Serialize for GlobSet {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let len = self.0.len();
        if len == 1 {
            serializer.serialize_str(self.0.first().unwrap().glob().glob())
        } else {
            let mut seq = serializer.serialize_seq(Some(len))?;
            for glob in self.0.iter() {
                seq.serialize_element(glob.glob().glob())?;
            }
            seq.end()
        }
    }
}

struct GlobSetVisitor();

impl<'de> Visitor<'de> for GlobSetVisitor {
    type Value = GlobSet;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a single or an array of globbing patterns")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: Error,
    {
        Ok(GlobSet(vec![
            Glob::new(v).map_err(Error::custom)?.compile_matcher(),
        ]))
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut ret = GlobSet(
            seq.size_hint()
                .map_or_else(|| Vec::new(), |size| Vec::with_capacity(size)),
        );
        while let Some(value) = seq.next_element::<&str>()? {
            ret.0
                .push(Glob::new(value).map_err(Error::custom)?.compile_matcher());
        }
        Ok(ret)
    }
}

/// [OneOrMany] like deserializer
impl<'de> Deserialize<'de> for GlobSet {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(GlobSetVisitor())
    }
}

impl Debug for GlobSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut ret = f.debug_list();
        for glob in self.0.iter() {
            ret.entry(&glob.glob().glob().to_string());
        }
        ret.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use serde_yaml_ng as yaml;

    #[test]
    fn basic() -> Result<()> {
        let ret = yaml::from_str::<GlobSet>("'*.c'")?;
        assert_eq!(1, ret.len());
        assert_eq!("'*.c'\n", yaml::to_string(&ret)?.as_str());

        let ret = yaml::from_str::<GlobSet>("['*.c', '**/test']")?;
        assert_eq!(2, ret.len());
        assert!(ret.is_match(&Path::new("test.c")));
        assert!(!ret.is_match(&Path::new("test.h")));
        assert_eq!("- '*.c'\n- '**/test'\n", yaml::to_string(&ret)?.as_str());

        tracing::debug!(?ret);
        Ok(())
    }
}
