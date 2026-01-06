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
** Created on: 2026-01-05T11:18:05
** Author: Sylvain Fargier <fargier.sylvain@gmail.com>
*/

use std::time::{Duration, SystemTime};

use super::{Info, Status};
use crate::utils::{IS_OUT_COLORED, tabled::TDisplay};
use colored::Colorize;

pub fn info_status_str(status: &Status) -> String {
    let str = format!("{status:?}");
    if IS_OUT_COLORED.get() {
        match status {
            Status::Created => str.bright_black().to_string(),
            Status::Running => str.green().to_string(),
            Status::Finished => str.bright_black().to_string(),
            Status::Stopped => str.bright_yellow().to_string(),
            Status::Crashed => str.red().to_string(),
        }
    } else {
        str
    }
}

pub fn info_duration_str(end_time: &Option<SystemTime>, info: &Info) -> String {
    (if let Some(end_time) = end_time {
        info.start_time
            .and_then(|start_time| end_time.duration_since(start_time).ok())
    } else {
        match info.status {
            Status::Running | Status::Stopped => info
                .start_time
                .and_then(|start_time| start_time.elapsed().ok()),
            _ => None,
        }
    })
    .map(|d| Duration::from_secs(d.as_secs()).to_string())
    .unwrap_or_else(String::new)
}

pub fn bytes_str(value: &u64) -> String {
    if value >= &0x4000_0000 {
        format!("{:.2} GiB", *value as f64 / 0x4000_0000 as f64)
    }
    else if value >= &0x10_0000 {
        format!("{:.2} MiB", *value as f64 / 0x10_0000 as f64)
    }
    else if value >= &0x400 {
        format!("{:.2} KiB", *value as f64 / 0x400 as f64)
    }
    else {
        format!("{:.2} B", value)
    }
}
