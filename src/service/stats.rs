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

use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Default)]
pub struct Stats {
    /// CPU usage in `%`
    pub cpu_usage: f32,
    /// total aggregated CPU time
    pub cpu_time: std::time::Duration,
    /// I/O read in [bytes/s]
    pub io_read: u64,
    /// total I/O read in [bytes]
    pub total_io_read: u64,
    /// I/O write in [bytes/s]
    pub io_write: u64,
    /// total I/O write in [bytes]
    pub total_io_write: u64,
    /// Memory resident size in [bytes]
    pub mem_rss: u64,
    /// Memory virtual size in [bytes]
    pub mem_vsz: u64,
    /// Uptime
    #[serde(
        with = "humantime_serde",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub uptime: Option<Duration>,
}
