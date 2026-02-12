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

#![allow(dead_code)]

pub mod size {
    use bytesize::ByteSize;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    /// Serializer for byte-sizes
    pub fn serialize<S>(value: &u64, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        ByteSize::b(*value).serialize(serializer)
        // Size::from_bytes(*value).serialize(serializer)
    }

    /// Deserializer for byte-size values
    pub fn deserialize<'de, D>(deserializer: D) -> Result<u64, D::Error>
    where
        D: Deserializer<'de>,
    {
        ByteSize::deserialize(deserializer).map(|v| v.as_u64())
        // Size::deserialize(deserializer).map(|v| v.bytes() as usize)
    }
}

pub mod duration {
    pub use humantime_serde::{deserialize, serialize};
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use anyhow::Result;
    use serde::{Deserialize, Serialize};
    use serde_yaml_ng as yaml;

    #[test]
    fn sizes() -> Result<()> {
        #[derive(Serialize, Deserialize)]
        struct Test {
            #[serde(with = "size")]
            size: u64,
        }

        for (test, result) in [
            ("12 KiB", 12 * 1024),
            ("12kb", 12000),
            ("12GiB", 12 * 1024 * 1024 * 1024),
            ("12G", 12_000_000_000),
            ("12.5 K", 12_500),
        ] {
            let value: Test = yaml::from_str(format!("size: \"{}\"", test).as_str())?;
            tracing::debug!(test, serialized = yaml::to_string(&value)?);
            assert_eq!(value.size, result, "{} != {}", test, result);
        }
        Ok(())
    }

    #[test]
    fn duration() -> Result<()> {
        #[derive(Serialize, Deserialize)]
        struct Test {
            #[serde(with = "duration")]
            duration: Duration,
        }

        for (test, result) in [
            ("12m", Duration::from_mins(12)),
            ("30s 120µs", Duration::from_micros(30000120)),
            ("3hours 12 minutes", Duration::from_mins(192)),
        ] {
            let value: Test = yaml::from_str(format!("duration: \"{}\"", test).as_str())?;
            tracing::debug!(test, serialized = yaml::to_string(&value)?);
            assert_eq!(value.duration, result, "{} != {:?}", test, result);
        }
        Ok(())
    }
}
