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

//! custom serializer/deserializer that supports [Option] serialization

use std::marker::PhantomData;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub trait Wrapper<T>: Sized {
    fn into(self) -> T;
    fn from(value: T) -> Self;

    /// construct wrapper from a convertible value
    ///
    /// the conversion is used either with:
    /// - `From<T> for T` as a noop
    /// - `From<&Option<T>> for Option<&T>` to build a `Wrapper<Option<&T>>` from `&Option<T>`
    fn from_any<K>(value: K) -> Self
    where
        K: Into<T>,
    {
        Self::from(value.into())
    }
}

impl<T, K> Wrapper<Option<T>> for Option<K>
where
    K: Wrapper<T>,
{
    fn into(self) -> Option<T> {
        self.map(Wrapper::into)
    }

    fn from(value: Option<T>) -> Self {
        value.map(Wrapper::from)
    }
}

pub struct WSerializer<W, K>(PhantomData<W>, PhantomData<K>);
impl<W, K> WSerializer<W, K>
where
    W: Wrapper<K>,
{
    pub fn serialize<V, S>(value: V, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        W: Serialize,
        V: Into<K>,
    {
        W::from_any(value).serialize(s)
    }
    pub fn deserialize<'de, V, D>(d: D) -> Result<V, D::Error>
    where
        D: Deserializer<'de>,
        W: Deserialize<'de> + Wrapper<V>,
    {
        W::deserialize(d).map(Wrapper::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use std::time::Duration;

    struct DurationWrapper<T>(T);
    impl<T> Wrapper<T> for DurationWrapper<T> {
        fn into(self) -> T {
            self.0
        }

        fn from(value: T) -> Self {
            Self(value)
        }
    }

    impl Serialize for DurationWrapper<&Duration> {
        fn serialize<S>(&self, serializer: S) -> std::prelude::v1::Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            self.0.as_secs().serialize(serializer)
        }
    }
    impl<'de> Deserialize<'de> for DurationWrapper<Duration> {
        fn deserialize<D>(deserializer: D) -> std::prelude::v1::Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            u64::deserialize(deserializer).map(|v| DurationWrapper(Duration::from_secs(v)))
        }
    }

    #[derive(Serialize, Deserialize)]
    struct Test {
        #[serde(with = "WSerializer::<DurationWrapper<_>, _>")]
        dur: Duration,
        #[serde(with = "WSerializer::<Option<DurationWrapper<_>>, _>", default)]
        opt_dur: Option<Duration>,
    }

    #[derive(Serialize, Deserialize)]
    struct SyntaxTest {
        #[serde(
            serialize_with = "WSerializer::<DurationWrapper<&Duration>, &Duration>::serialize",
            deserialize_with = "WSerializer::<DurationWrapper<Duration>, Duration>::deserialize"
        )]
        dur: Duration,
    }
    #[test]
    fn serde_duration() -> Result<()> {
        let value: Test = serde_json::from_str(r#"{"dur": 123, "opt_dur": 234}"#)?;
        assert_eq!(value.dur, Duration::from_secs(123));
        assert_eq!(value.opt_dur, Some(Duration::from_secs(234)));
        Ok(())
    }
}
