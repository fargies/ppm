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
** Created on: 2026-01-03T00:21:32
** Author: Sylvain Fargier <fargier.sylvain@gmail.com>
*/

use libc::{timer_t, sigevent, itimerspec ,timer_create, timer_settime, timer_delete};
use std::{ptr::null_mut, time::Duration};
use anyhow::Result;

use super::libc_check;

/// Signal based POSIX timer
///
/// raises a `Signal(ALRM)` signal on expiry
pub struct Timer {
    id: timer_t,
    timerspec: itimerspec,
}

impl Default for Timer {
    fn default() -> Self {
        let mut timer_id: timer_t = unsafe { std::mem::zeroed() };
        let mut sigev: sigevent = unsafe { std::mem::zeroed() };
        sigev.sigev_notify = libc::SIGEV_SIGNAL;
        sigev.sigev_signo = libc::SIGALRM;

        libc_check(unsafe { timer_create(libc::CLOCK_MONOTONIC, &mut sigev, &mut timer_id) })
            .unwrap();

        Self {
            id: timer_id,
            timerspec: unsafe { std::mem::zeroed::<itimerspec>() },
        }
    }
}

impl Timer {
    /// Create a new timer
    pub fn new(duration: Duration, repeat: bool) -> Self {
        let mut ret = Timer::default();
        ret.set_duration(duration);
        if repeat {
            ret.set_interval(duration);
        }
        ret
    }

    /// Set timer duration
    pub fn set_duration(&mut self, duration: Duration) -> &mut Self {
        self.timerspec.it_value.tv_sec = duration.as_secs() as i64;
        self.timerspec.it_value.tv_nsec = duration.subsec_nanos().into();
        self
    }

    /// Set interval
    pub fn set_interval(&mut self, duration: Duration) -> &mut Self {
        self.timerspec.it_interval.tv_sec = duration.as_secs() as i64;
        self.timerspec.it_interval.tv_nsec = duration.subsec_nanos().into();
        self
    }

    /// Retrieve the timer duration
    pub fn duration(&self) -> Duration {
        Duration::from_secs(self.timerspec.it_value.tv_sec as u64)
            + Duration::from_nanos(self.timerspec.it_value.tv_nsec as u64)
    }

    /// Retrieve the timer interval
    pub fn interval(&self) -> Duration {
        Duration::from_secs(self.timerspec.it_interval.tv_sec as u64)
            + Duration::from_nanos(self.timerspec.it_interval.tv_nsec as u64)
    }

    /// Start the system timer
    pub fn start(&self) -> Result<()> {
        libc_check(unsafe { timer_settime(self.id, 0, &self.timerspec, null_mut()) })
    }

    /// Stop the system timer
    ///
    /// Sets both interval and duration to zero on the system side
    pub fn stop(&self) -> Result<()> {
        let val = unsafe { std::mem::zeroed::<itimerspec>() };
        libc_check(unsafe { timer_settime(self.id, 0, &val, null_mut()) })
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        libc_check(unsafe { timer_delete(self.id) }).unwrap();
    }
}
