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
use serde::{Deserializer, Serializer};

static REF_INSTANT: RwLock<Option<(Instant, SystemTime)>> = RwLock::new(None);
const MAX_DRIFT: Duration = Duration::from_millis(10);

#[tracing::instrument()]
fn update_ref_instant() -> (Instant, SystemTime) {
    tracing::info!("updating time reference");
    let ref_time = (Instant::now(), SystemTime::now());
    *REF_INSTANT.write().unwrap() = Some(ref_time);
    ref_time
}

/// Check reference instant/systime drift
///
/// returns false if reference time was updated
pub fn check_ref_time() -> bool {
    let (check_instant, check_time) = (Instant::now(), SystemTime::now());
    let converted_instant = from_systime(&check_time);

    let drift = if converted_instant >= check_instant {
        converted_instant.duration_since(check_instant)
    } else {
        check_instant.duration_since(converted_instant)
    };
    if drift >= MAX_DRIFT {
        tracing::warn!(
            drift = humantime::format_duration(drift).to_string(),
            "clock drift detected"
        );
        update_ref_instant();
        false
    } else {
        true
    }
}

pub fn from_systime(systime: &SystemTime) -> Instant {
    let ref_instant = *REF_INSTANT.read().unwrap();
    let ref_instant = ref_instant.unwrap_or_else(update_ref_instant);
    if &ref_instant.1 >= systime {
        let duration = ref_instant
            .1
            .duration_since(*systime)
            .unwrap_or(Duration::ZERO);
        ref_instant.0.checked_sub(duration).unwrap()
    } else {
        let duration = systime
            .duration_since(ref_instant.1)
            .unwrap_or(Duration::ZERO);
        ref_instant.0.checked_add(duration).unwrap()
    }
}

pub fn to_systime(instant: &Instant) -> SystemTime {
    let ref_instant = *REF_INSTANT.read().unwrap();
    let ref_instant = ref_instant.unwrap_or_else(update_ref_instant);
    if &ref_instant.0 >= instant {
        let duration = ref_instant.0.duration_since(*instant);
        ref_instant.1.checked_sub(duration).unwrap()
    } else {
        let duration = instant.duration_since(ref_instant.0);
        ref_instant.1.checked_add(duration).unwrap()
    }
}

pub fn serialize<S>(instant: &Instant, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    humantime_serde::serialize(&to_systime(instant), serializer)
}

pub fn deserialize<'de, D>(deserializer: D) -> Result<Instant, D::Error>
where
    D: Deserializer<'de>,
{
    humantime_serde::deserialize(deserializer).map(|timestamp: SystemTime| from_systime(&timestamp))
}

pub mod option {
    use super::*;

    pub fn serialize<S>(instant: &Option<Instant>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        humantime_serde::serialize(&instant.as_ref().map(to_systime), serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Instant>, D::Error>
    where
        D: Deserializer<'de>,
    {
        humantime_serde::deserialize(deserializer)
            .map(|timestamp: SystemTime| Some(from_systime(&timestamp)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;

    #[test]
    fn serde() -> Result<()> {
        let now = Instant::now();
        let systime = to_systime(&now);
        std::thread::sleep(Duration::from_millis(10));
        update_ref_instant();
        assert_eq!(
            systime.elapsed()?.as_millis(),
            to_systime(&now).elapsed()?.as_millis()
        );
        Ok(())
    }
}
