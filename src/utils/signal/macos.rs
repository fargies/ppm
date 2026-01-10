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
** Created on: 2026-01-02T22:19:29
** Author: Sylvain Fargier <fargier.sylvain@gmail.com>
*/

#![allow(dead_code)]

use anyhow::Result;
use std::time::Duration;
use std::{ops::Deref, os::raw::c_void};

use super::libc_check;
use dispatchr::{QoS, queue, source, time::Time};

pub struct Timer {
    source: source::Managed,
    _interval: Duration,
    _duration: Duration,
}

impl Default for Timer {
    fn default() -> Self {
        let queue = queue::global(QoS::Default).unwrap();
        let tid = unsafe { libc::pthread_self() };
        let source = source::Managed::create(source::dispatch_source_type_t::timer(), 0, 0, queue);
        unsafe {
            dispatch_set_context(
                source.deref() as *const _ as *const c_void,
                tid as *mut c_void,
            );
        }
        source.set_event_handler_f(dispatch_function);
        tracing::trace!(tid, "timer created");
        Self {
            source,
            _interval: Duration::ZERO,
            _duration: Duration::ZERO,
        }
    }
}

unsafe extern "C" {
    fn dispatch_set_context(object: *const c_void, context: *mut c_void);
}

extern "C" fn dispatch_function(_arg: *mut c_void) {
    let tid = _arg as libc::pthread_t;
    tracing::trace!(tid, "timer dispatched sending SIGALRM");
    libc_check(unsafe { libc::pthread_kill(_arg as libc::pthread_t, libc::SIGALRM) }).unwrap();
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
        self._duration = duration;
        self
    }

    /// Set interval
    pub fn set_interval(&mut self, interval: Duration) -> &mut Self {
        self._interval = interval;
        self
    }

    /// Retrieve the timer duration
    pub fn duration(&self) -> Duration {
        self._duration
    }

    /// Retrieve the timer interval
    pub fn interval(&self) -> Duration {
        self._interval
    }

    /// Start the system timer
    pub fn start(&self) -> Result<()> {
        self.source.set_timer(
            Time::NOW.new_after(self._duration.as_nanos() as i64),
            self._interval.as_nanos() as u64,
            1_000_000,
        );
        self.source.resume();
        Ok(())
    }

    /// Stop the system timer
    ///
    /// Sets both interval and duration to zero on the system side
    pub fn stop(&self) -> Result<()> {
        self.source.cancel();
        Ok(())
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        self.source.cancel();
    }
}
