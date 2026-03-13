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
** Created on: 2026-01-01T15:15:06
** Author: Sylvain Fargier <fargier.sylvain@gmail.com>
*/

#![allow(dead_code)]

use std::{
    sync::RwLock,
    time::{Duration, Instant, SystemTime},
};

use humantime_serde;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

static REF_INSTANT: RwLock<Option<RefTimePoint>> = RwLock::new(None);
const MAX_DRIFT: Duration = Duration::from_millis(10);

#[derive(Copy, Clone)]
pub struct RefTimePoint {
    pub instant: Instant,
    pub systime: SystemTime,
}

impl RefTimePoint {
    #[inline]
    #[allow(clippy::new_without_default)]
    fn new() -> Self {
        Self {
            instant: Instant::now(),
            systime: SystemTime::now(),
        }
    }

    #[inline]
    pub fn get() -> Self {
        let ref_instant = *REF_INSTANT.read().unwrap();
        ref_instant.unwrap_or_else(Self::update)
    }

    /// Update global reference time point
    ///
    /// returns the new [RefTimePoint]
    #[tracing::instrument()]
    pub fn update() -> Self {
        let ref_time = Self::new();
        *REF_INSTANT.write().unwrap() = Some(ref_time);
        ref_time
    }

    /// Check reference instant/systime drift
    ///
    /// returns false if reference time was updated
    pub fn check(max_drift: Option<Duration>) -> bool {
        let new_ref = Self::new();
        let converted_instant = from_systime(&new_ref.systime);

        let drift = if converted_instant >= new_ref.instant {
            converted_instant.duration_since(new_ref.instant)
        } else {
            new_ref.instant.duration_since(converted_instant)
        };
        if drift >= max_drift.unwrap_or(MAX_DRIFT) {
            tracing::warn!(
                drift = humantime::format_duration(drift).to_string(),
                "clock drift detected"
            );
            *REF_INSTANT.write().unwrap() = Some(new_ref);
            false
        } else {
            true
        }
    }
}

/// Convert [SystemTime] to [Instant] using a reference time-point
pub fn from_systime(systime: &SystemTime) -> Instant {
    let tp = RefTimePoint::get();
    if &tp.systime >= systime {
        let duration = tp
            .systime
            .duration_since(*systime)
            .unwrap_or(Duration::ZERO);
        tp.instant.checked_sub(duration).unwrap()
    } else {
        let duration = systime.duration_since(tp.systime).unwrap_or(Duration::ZERO);
        tp.instant.checked_add(duration).unwrap()
    }
}

/// Convert [Instant] to [SystemTime] using a reference time-point
pub fn to_systime(instant: &Instant) -> SystemTime {
    let tp = RefTimePoint::get();
    if &tp.instant >= instant {
        let duration = tp.instant.duration_since(*instant);
        tp.systime.checked_sub(duration).unwrap()
    } else {
        let duration = instant.duration_since(tp.instant);
        tp.systime.checked_add(duration).unwrap()
    }
}

/// Serializer for [Instant]
///
/// add `#[serde(with=utils::serializers::instant)]` to use this module
pub fn serialize<S, V>(instant: &V, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    for<'a> Serde<&'a V>: Serialize,
{
    Serde(instant).serialize(serializer)
}

/// Deserializer for [Instant]
pub fn deserialize<'de, D, V>(deserializer: D) -> Result<V, D::Error>
where
    D: Deserializer<'de>,
    Serde<V>: Deserialize<'de>,
{
    Serde::deserialize(deserializer).map(|v| v.0)
}

pub struct Serde<T>(T);

impl<'de> Deserialize<'de> for Serde<Instant> {
    fn deserialize<D>(deserializer: D) -> Result<Serde<Instant>, D::Error>
    where
        D: Deserializer<'de>,
    {
        humantime_serde::deserialize(deserializer)
            .map(|timestamp: SystemTime| Serde(from_systime(&timestamp)))
    }
}

impl<'de> Deserialize<'de> for Serde<Option<Instant>> {
    fn deserialize<D>(deserializer: D) -> Result<Serde<Option<Instant>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        humantime_serde::deserialize(deserializer)
            .map(|timestamp: Option<SystemTime>| Serde(timestamp.map(|v| from_systime(&v))))
    }
}

impl Serialize for Serde<&Instant> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        humantime_serde::serialize(&to_systime(self.0), serializer)
    }
}

impl Serialize for Serde<&Option<Instant>> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        humantime_serde::serialize(&self.0.as_ref().map(to_systime), serializer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use serde::{Deserialize, Serialize};
    use serde_yaml_ng as yaml;
    use std::time::Duration;

    #[test]
    fn convert() -> Result<()> {
        RefTimePoint::check(Some(Duration::from_micros(500)));
        let ref_instant = Instant::now();
        let ref_systime = to_systime(&ref_instant);
        std::thread::sleep(Duration::from_millis(10));
        RefTimePoint::update();
        let now = SystemTime::now();
        tracing::trace!(?ref_instant, ?ref_systime, new_ref_systime = ?to_systime(&ref_instant));
        assert_eq!(
            now.duration_since(ref_systime)?.as_millis(),
            now.duration_since(to_systime(&ref_instant))?.as_millis(),
        );
        Ok(())
    }

    #[test]
    fn serde_option() -> Result<()> {
        #[derive(Serialize, Deserialize)]
        struct Test {
            #[serde(with = "super", default)]
            value: Option<Instant>,
        }

        let value: Test = yaml::from_str("{}")?;
        assert_eq!(None, value.value);

        let now = Instant::now();
        let value: Test = yaml::from_str(yaml::to_string(&Test { value: Some(now) })?.as_str())?;
        assert_eq!(
            value.value.unwrap().elapsed().as_millis(),
            now.elapsed().as_millis()
        );
        Ok(())
    }
}
