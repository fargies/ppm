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

use std::collections::VecDeque;

use serde::{Deserialize, Deserializer, Serialize, Serializer, ser::SerializeSeq};

#[derive(Deserialize)]
#[serde(untagged)]
pub enum OneOrMany<K>
where
    K: IntoIterator,
    K: FromIterator<K::Item>,
{
    One(K::Item),
    Many(K),
}

impl<K> OneOrMany<K>
where
    K: IntoIterator,
    K: FromIterator<K::Item>,
{
    pub fn into_inner(self) -> K {
        match self {
            OneOrMany::One(item) => K::from_iter([item; 1]),
            OneOrMany::Many(k) => k,
        }
    }
}

pub struct OneOrManyWrapper<T>(pub T);

impl<T> OneOrManyWrapper<T> {
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<'a, V> OneOrManyWrapper<&'a V>
where
    &'a V: IntoIterator,
    <&'a V as IntoIterator>::Item: Serialize,
{
    pub fn serialize_one_or_many<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut it = self.0.into_iter();
        let size_hint = it.size_hint().1;
        if let Some(first) = it.next() {
            if let Some(second) = it.next() {
                let mut seq = serializer.serialize_seq(size_hint)?;
                seq.serialize_element(&first)?;
                seq.serialize_element(&second)?;
                for elt in it {
                    seq.serialize_element(&elt)?;
                }
                seq.end()
            } else {
                first.serialize(serializer)
            }
        } else {
            serializer.serialize_seq(Some(0))?.end()
        }
    }
}

impl<V> Serialize for OneOrManyWrapper<&Vec<V>>
where
    V: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.serialize_one_or_many(serializer)
    }
}

impl<V> Serialize for OneOrManyWrapper<&VecDeque<V>>
where
    V: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.serialize_one_or_many(serializer)
    }
}

impl<'de, K> Deserialize<'de> for OneOrManyWrapper<Vec<K>>
where
    K: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        OneOrMany::deserialize(deserializer).map(|m| OneOrManyWrapper(m.into_inner()))
    }
}

impl<'de, K> Deserialize<'de> for OneOrManyWrapper<VecDeque<K>>
where
    K: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        OneOrMany::deserialize(deserializer).map(|m| OneOrManyWrapper(m.into_inner()))
    }
}

pub struct Serde<T>(T);

impl<'a, V> Serialize for Serde<&'a V>
where
    OneOrManyWrapper<&'a V>: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        OneOrManyWrapper(self.0).serialize(serializer)
    }
}

impl<'a, V> Serialize for Serde<&'a Option<V>>
where
    OneOrManyWrapper<&'a V>: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.0.as_ref() {
            Some(v) => serializer.serialize_some(&OneOrManyWrapper(v)),
            None => serializer.serialize_none(),
        }
    }
}

impl<'de, K> Deserialize<'de> for Serde<K>
where
    OneOrManyWrapper<K>: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        OneOrManyWrapper::<K>::deserialize(deserializer).map(|m| Serde(m.into_inner()))
    }
}

impl<'de, K> Deserialize<'de> for Serde<Option<K>>
where
    OneOrManyWrapper<K>: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<OneOrManyWrapper<K>>::deserialize(deserializer)
            .map(|m| Serde(m.map(|m| m.into_inner())))
    }
}

/// OneOrMany serializer
///
/// add `#[serde(with=utils::serializers::instant)]` to use this module
pub fn serialize<S, V>(value: &V, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    for<'a> Serde<&'a V>: Serialize,
{
    Serde(value).serialize(serializer)
}

/// Deserializer for [Instant]
pub fn deserialize<'de, D, V>(deserializer: D) -> Result<V, D::Error>
where
    D: Deserializer<'de>,
    Serde<V>: Deserialize<'de>,
{
    Serde::deserialize(deserializer).map(|m| m.0)
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use anyhow::Result;
    use serde::{Deserialize, Serialize};
    use serde_yaml_ng as yaml;

    #[test]
    fn one_or_many() -> Result<()> {
        #[derive(Deserialize, Serialize, PartialEq, Debug)]
        struct OneOrManyTest {
            #[serde(with = "super")]
            pub value: Vec<i32>,
        }

        for (yml_str, ref_value) in [
            ("1", [1].as_slice()),
            ("[1, 2, 3]", &[1, 2, 3]),
            ("[1]", &[1]),
        ] {
            let v: OneOrManyTest = yaml::from_str(format!("value: {yml_str}").as_str())?;
            let ser = yaml::to_string(&v)?;
            let v: OneOrManyTest = yaml::from_str(ser.as_str())?;

            assert_eq!(
                OneOrManyTest {
                    value: Vec::from(ref_value)
                },
                v
            );
        }

        assert_eq!(
            "value: 1\n",
            yaml::to_string(&OneOrManyTest { value: vec![1] })?.as_str()
        );

        Ok(())
    }

    #[test]
    fn one_or_many_types() -> Result<()> {
        for (yml_str, ref_value) in [
            ("1", [1].as_slice()),
            ("[1, 2, 3]", &[1, 2, 3]),
            ("[1]", &[1]),
        ] {
            let value: VecDeque<i32> = super::deserialize(yaml::Deserializer::from_str(yml_str))?;
            let mut vec = Vec::with_capacity(128);
            super::serialize(&value, &mut yaml::Serializer::new(&mut vec))?;
            let mut value: VecDeque<i32> =
                super::deserialize(yaml::Deserializer::from_slice(vec.as_slice()))?;
            assert_eq!(ref_value, value.make_contiguous());
        }

        let value: VecDeque<i32> = super::deserialize(yaml::Deserializer::from_str("1"))?;
        let mut vec = Vec::with_capacity(128);
        super::serialize(&value, &mut yaml::Serializer::new(&mut vec))?;
        assert_eq!("1\n", String::from_utf8(vec).unwrap());
        Ok(())
    }

    #[test]
    fn one_or_many_option_types() -> Result<()> {
        #[derive(Serialize, Deserialize)]
        struct Test {
            #[serde(with = "super", default)]
            opt: Option<VecDeque<i32>>,
        }

        let test: Test = yaml::from_str("{}")?;
        assert_eq!(test.opt, None);
        assert_eq!("opt: null\n", yaml::to_string(&test)?.as_str());
        Ok(())
    }
}
