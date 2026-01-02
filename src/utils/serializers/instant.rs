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

use std::{
    sync::RwLock,
    time::{Duration, Instant, SystemTime},
};

use humantime_serde;
use serde::{Deserializer, Serializer};

static REF_INSTANT: RwLock<Option<(Instant, SystemTime)>> = RwLock::new(None);
const MAX_REF_AGE: Duration = Duration::from_hours(1);

#[tracing::instrument(ret)]
fn update_ref_instant() {
    *REF_INSTANT.write().unwrap() = Some((Instant::now(), SystemTime::now()));
}

fn from_systime(systime: &SystemTime) -> Instant {
    if let Some(ref_instant) = *REF_INSTANT.read().unwrap() {
        let duration = systime
            .duration_since(ref_instant.1)
            .unwrap_or(Duration::ZERO);
        if duration <= MAX_REF_AGE {
            return ref_instant.0.checked_add(duration).unwrap();
        }
    }
    update_ref_instant();
    from_systime(systime)
}

fn to_systime(instant: &Instant) -> SystemTime {
    if let Some(ref_instant) = *REF_INSTANT.read().unwrap() {
        let duration = instant.duration_since(ref_instant.0);
        if duration <= MAX_REF_AGE {
            return ref_instant.1.checked_add(duration).unwrap();
        }
    }
    update_ref_instant();
    to_systime(instant)
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

#[cfg(test)]
mod tests {
    #[test]
    fn serde() {

    }
}
