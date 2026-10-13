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
** Created on: 2025-12-22T16:55:59
** Author: Sylvain Fargier <fargier.sylvain@gmail.com>
*/

use crate::utils::{
    serde_utils::wrapper::{WSerializer, Wrapper},
    serializers::tabled::TDisplay,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tabled::Tabled;

use super::tabled::bytes_str;

struct MicroDurationWrapper<T>(T);
impl<T> Wrapper<T> for MicroDurationWrapper<T> {
    fn into(self) -> T {
        self.0
    }

    fn from(value: T) -> Self {
        Self(value)
    }
}

impl Serialize for MicroDurationWrapper<&Duration> {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        (self.0.as_micros() as u64).serialize(s)
    }
}

impl<'de> Deserialize<'de> for MicroDurationWrapper<Duration> {
    fn deserialize<D>(d: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        u64::deserialize(d).map(|v| Self(Duration::from_micros(v)))
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Default, Tabled)]
pub struct Stats {
    /// CPU usage in `%`
    #[tabled(rename = "CPU", format = "{:.2} %")]
    pub cpu_usage: f32,

    /// total aggregated CPU time
    #[serde(with = "WSerializer::<MicroDurationWrapper<_>, _>")]
    #[tabled(display("TDisplay::to_string"), rename = "CPU time")]
    pub cpu_time: Duration,

    /// I/O read in [bytes/s]
    #[tabled(rename = "I/O read", format("{}/sec", bytes_str(&self.io_read)))]
    pub io_read: u64,

    /// total I/O read in [bytes]
    #[tabled(rename = "I/O read", display = "bytes_str")]
    pub total_io_read: u64,

    /// I/O write in [bytes/s]
    #[tabled(rename = "I/O write", format("{}/sec", bytes_str(&self.io_write)))]
    pub io_write: u64,

    /// total I/O write in [bytes]
    #[tabled(rename = "I/O write", display = "bytes_str")]
    pub total_io_write: u64,

    /// Memory resident size in [bytes]
    #[tabled(rename = "Mem RSS", display = "bytes_str")]
    pub mem_rss: u64,

    /// Memory virtual size in [bytes]
    #[tabled(rename = "Mem VSZ", display = "bytes_str")]
    pub mem_vsz: u64,

    /// Uptime
    #[serde(
        with = "WSerializer::<Option<MicroDurationWrapper<_>>, _>",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    #[tabled(display("TDisplay::to_string"), rename = "uptime")]
    pub uptime: Option<Duration>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use serde_json as json;

    #[test]
    fn serde() -> Result<()> {
        let stats = Stats::default();
        let str = json::to_string(&stats)?;
        json::from_str::<Stats>(&str)?;
        Ok(())
    }
}
