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
** Created on: 2025-12-22T16:25:07
** Author: Sylvain Fargier <fargier.sylvain@gmail.com>
*/

use serde::{Deserialize, Serialize};
use tabled::{Tabled, derive::display};
use std::time::SystemTime;
use crate::utils::tabled::TabledDisplay;

use super::Status;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Tabled)]
pub struct Info {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[tabled(display("display::option", ""))]
    pub pid: Option<libc::pid_t>,
    pub active: bool,
    #[tabled(display("display::debug"))]
    pub status: Status,
    #[serde(
        with = "humantime_serde",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    #[tabled(display("TabledDisplay::to_string"))]
    pub start_time: Option<SystemTime>,
    #[serde(
        with = "humantime_serde",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    #[tabled(display("TabledDisplay::to_string"))]
    pub end_time: Option<SystemTime>,
    #[serde(default)]
    #[tabled(rename = "↺")]
    pub restarts: usize,
}

impl Default for Info {
    fn default() -> Self {
        Self {
            pid: None,
            active: true,
            status: Status::Created,
            start_time: None,
            end_time: None,
            restarts: 0,
        }
    }
}

impl Info {
    pub fn set_running(&mut self, pid: libc::pid_t) {
        match self.status {
            Status::Created | Status::Finished | Status::Crashed => {
                tracing::info!("{:?} -> {:?}", self.status, Status::Running);
                self.pid = Some(pid);
                self.start_time = Some(std::time::SystemTime::now());
                self.restarts += 1;
                self.status = Status::Running;
                self.end_time = None;
            }
            Status::Running => {
                self.pid = Some(pid);
            }
            Status::Stopped => {
                tracing::info!("{:?} -> {:?}", self.status, Status::Running);
                self.pid = Some(pid);
                self.status = Status::Running;
                self.end_time = None;
            }
        }
    }

    pub fn set_finished(&mut self) {
        match self.status {
            Status::Running | Status::Stopped => {
                tracing::info!("{:?} -> {:?}", self.status, Status::Finished);
                self.pid = None;
                self.status = Status::Finished;
                self.end_time = Some(std::time::SystemTime::now());
            }
            Status::Finished => {}
            _ => tracing::warn!(
                status = ?self.status,
                next = ?Status::Finished,
                "invalid process transition"
            ),
        }
    }

    pub fn set_stopped(&mut self) {
        match self.status {
            Status::Running => {
                tracing::info!("{:?} -> {:?}", self.status, Status::Stopped);
                self.status = Status::Stopped;
                self.end_time = Some(std::time::SystemTime::now());
            }
            Status::Stopped => {}
            _ => tracing::warn!(
                status = ?self.status,
                next = ?Status::Stopped,
                "invalid process transition"
            ),
        }
    }

    pub fn set_crashed(&mut self) {
        match self.status {
            Status::Running | Status::Stopped => {
                tracing::warn!("{:?} -> {:?}", self.status, Status::Crashed);
                self.pid = None;
                self.status = Status::Crashed;
                self.end_time = Some(std::time::SystemTime::now());
            }
            Status::Crashed => {}
            _ => tracing::warn!(
                status = ?self.status,
                next = ?Status::Crashed,
                "invalid process transition"
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde() {
        let data = "active: true\nstatus: Created\nrestarts: 0\n";
        let info = Info::default();
        assert_eq!(data, serde_yaml_ng::to_string(&info).unwrap());
        assert_eq!(serde_yaml_ng::from_str::<Info>(data).unwrap(), info);
    }
}
