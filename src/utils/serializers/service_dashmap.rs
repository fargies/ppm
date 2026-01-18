/*
** Copyright (C) 2026 Sylvain Fargier
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
** Created on: 2026-01-04T10:05:52
** Author: Sylvain Fargier <fargier.sylvain@gmail.com>
*/

use std::{collections::BTreeSet, sync::Arc};

use dashmap::DashMap;
use serde::{
    Deserializer, Serialize, Serializer,
    de::{Error, Visitor},
};

use crate::{
    service::{SERVICE_ID_INVALID, Service, ServiceId},
    utils::serde_utils::{InnerRef, wrap_seq_iterator},
};

type ServiceMap = DashMap<ServiceId, Arc<Service>>;

pub fn serialize<S>(services: &ServiceMap, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    /* we want it ordered, let's first iter on Ids */
    let keys: BTreeSet<ServiceId> = services.iter().map(|it| *it.key()).collect();
    wrap_seq_iterator(
        keys.iter()
            .filter_map(|key| services.get(key).map(|it| InnerRef(it, |r| r.value()))),
    )
    .serialize(serializer)
}

pub fn deserialize<'de, D>(deserializer: D) -> Result<ServiceMap, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_any(ServiceMapVisitor())
}

struct ServiceMapVisitor();

impl<'de> Visitor<'de> for ServiceMapVisitor {
    type Value = ServiceMap;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("expecting an array of `Service`")
    }

    fn visit_map<A>(self, mut access: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::MapAccess<'de>,
    {
        let map = ServiceMap::with_capacity(access.size_hint().unwrap_or(0));

        while let Some((key, mut value)) = access.next_entry::<ServiceId, Service>()? {
            if value.id != SERVICE_ID_INVALID && value.id != key {
                return Err(Error::custom(
                    "service `id` must shall not be set when using a service",
                ));
            } else {
                value.id = key;
            }
            value = value.validate().map_err(Error::custom)?;
            map.insert(key, Arc::new(value));
        }
        Ok(map)
    }

    fn visit_seq<A>(self, mut access: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::SeqAccess<'de>,
    {
        let mut vec = Vec::<Service>::with_capacity(access.size_hint().unwrap_or(0));

        while let Some(value) = access.next_element::<Service>()? {
            vec.push(value.validate().map_err(Error::custom)?);
        }
        Ok(DashMap::from_iter(
            vec.into_iter().map(|srv| (srv.id, Arc::new(srv))),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use serde::{Deserialize, Serialize};
    use serde_yaml_ng as yaml;

    #[derive(Serialize, Deserialize, Debug)]
    struct Wrapper {
        #[serde(with = "super")]
        services: ServiceMap,
    }

    #[test]
    fn serde() -> Result<()> {
        assert!(
            yaml::from_str::<Wrapper>("services: {}")?
                .services
                .is_empty()
        );

        yaml::from_str::<Wrapper>(
            r#"services:
- name: test
  command: { path: ls }"#,
        )?;

        yaml::from_str::<Wrapper>("services: [ { name: test, command: { path: ls } } ]")?;
        yaml::from_str::<Wrapper>("services: { 0: { name: test, command: { path: ls } } }")?;
        yaml::from_str::<Wrapper>("services: { 0: { id: 0, name: test, command: { path: ls } } }")?;
        Ok(())
    }

    #[test]
    fn validation_errors() -> Result<()> {
        let data = [
            ("services: [ { command: { path: ls } } ]", "name"),
            ("services: [ { name: test } ]", "command"),
            (
                "services: { 0: { id: 1, name: test, command: { path: ls } } }",
                "id",
            ),
        ];
        for test in data.into_iter() {
            let err = yaml::from_str::<Wrapper>(test.0).expect_err("should have failed to parse");
            if !err.to_string().contains(test.1) {
                panic!("invalid error string \"{:}\" for \"{:}\"", err, test.0);
            }
        }

        Ok(())
    }
}
