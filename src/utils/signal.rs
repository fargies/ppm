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
** Created on: 2025-12-27T19:28:19
** Author: Sylvain Fargier <fargier.sylvain@gmail.com>
*/

use anyhow::Result;
use std::{fmt::Debug, ops::Deref, ptr::null_mut, sync::LazyLock};

/// POSIX Signal wrapper
#[derive(Clone, Copy, PartialEq)]
pub struct Signal(pub libc::c_int);

pub const SIGALRM: Signal = Signal(libc::SIGALRM);
pub const SIGCHLD: Signal = Signal(libc::SIGCHLD);
pub const SIGTERM: Signal = Signal(libc::SIGTERM);
pub const SIGSTOP: Signal = Signal(libc::SIGSTOP);
pub const SIGKILL: Signal = Signal(libc::SIGKILL);

static FULL_SET: LazyLock<SignalSet> = LazyLock::new(|| {
    let mut sigset: libc::sigset_t = 0;
    unsafe {
        libc_check(libc::sigfillset(&mut sigset)).unwrap();
        // remove signals that can't be controlled from the set
        libc_check(libc::sigdelset(&mut sigset, libc::SIGSTOP)).unwrap();
        libc_check(libc::sigdelset(&mut sigset, libc::SIGKILL)).unwrap();
        libc_check(libc::sigdelset(&mut sigset, 32)).unwrap();
    }
    SignalSet(sigset)
});

impl Signal {
    pub fn kill(pid: libc::pid_t, signal: Signal) -> Result<()> {
        unsafe { libc_check(libc::kill(pid, *signal)) }
    }

    pub fn set_handler(&self, handler: usize) -> Result<()> {
        let ret = unsafe { libc::signal(self.0, handler) };
        libc_check(if ret == libc::SIG_ERR { -1 } else { 0 })
    }
}

impl Deref for Signal {
    type Target = libc::c_int;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Debug for Signal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // f.write_str("Signal(")?;
        match self.0 {
            libc::SIGALRM => f.write_str("SIGALRM"),
            libc::SIGCHLD => f.write_str("SIGCHLD"),
            libc::SIGTERM => f.write_str("SIGTERM"),
            libc::SIGSTOP => f.write_str("SIGSTOP"),
            libc::SIGKILL => f.write_str("SIGKILL"),
            sig => write!(f, "SIG({})", sig),
        }
        // f.write_str(")")
    }
}

/// assert for libc functions
fn libc_check(res: libc::c_int) -> Result<()> {
    if res != 0 {
        let err = std::io::Error::last_os_error();
        tracing::trace_span!("libc_check", ?err);
        Err(err.into())
    } else {
        Ok(())
    }
}

pub struct SignalSet(pub libc::sigset_t);

impl Debug for SignalSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("SignalSet")
            .field(&format_args!("{:X}", self.0))
            .finish()
    }
}

impl SignalSet {
    /// Block signals in the set
    #[tracing::instrument(level = "TRACE")]
    pub fn block(&self) -> Result<()> {
        unsafe { libc_check(libc::sigprocmask(libc::SIG_BLOCK, &self.0, null_mut())) }
    }

    /// Unblock signals in the set
    #[tracing::instrument(level = "TRACE")]
    pub fn unblock(&self) -> Result<()> {
        unsafe { libc_check(libc::sigprocmask(libc::SIG_UNBLOCK, &self.0, null_mut())) }
    }

    /// Wait for a (blocked) signal in the set to raise
    pub fn wait(&self) -> Result<Signal> {
        unsafe {
            let mut sig: libc::c_int = 0;
            libc_check(libc::sigwait(&self.0, &mut sig))?;
            Ok(Signal(sig))
        }
    }

    pub fn fill(&mut self) -> &mut Self {
        self.0 = FULL_SET.0;
        self
    }

    pub fn iter<'a>(&'a self) -> SignalSetIterator<'a> {
        SignalSetIterator {
            index: 0,
            sigset: self,
        }
    }

    #[tracing::instrument(level = "TRACE")]
    pub fn restore(&self) -> Result<()> {
        for sig in self {
            sig.set_handler(libc::SIG_DFL)
                .inspect_err(|err| tracing::error!(?sig, ?err, "failed to reset handler"))?;
        }
        self.unblock()
    }
}

impl Default for SignalSet {
    fn default() -> Self {
        let mut set: libc::sigset_t = 0;
        unsafe {
            libc_check(libc::sigemptyset(&mut set)).unwrap();
        }
        Self(set)
    }
}

impl std::ops::Add<Signal> for SignalSet {
    type Output = SignalSet;

    /// Add a signal in the set
    fn add(self, rhs: Signal) -> Self::Output {
        unsafe {
            let mut ret = self;
            libc_check(libc::sigaddset(&mut ret.0, *rhs)).unwrap();
            ret
        }
    }
}

impl std::ops::Sub<Signal> for SignalSet {
    type Output = SignalSet;

    fn sub(self, rhs: Signal) -> Self::Output {
        unsafe {
            let mut ret = self;
            libc_check(libc::sigdelset(&mut ret.0, *rhs)).unwrap();
            ret
        }
    }
}

pub struct SignalSetIterator<'a> {
    index: u8,
    sigset: &'a SignalSet,
}

impl Iterator for SignalSetIterator<'_> {
    type Item = Signal;

    fn next(&mut self) -> Option<Self::Item> {
        for i in self.index..32 {
            if (self.sigset.0 & (1 << i)) != 0 {
                self.index = i + 1;
                return Some(Signal(self.index as i32));
            }
        }
        None
    }
}

impl<'a> IntoIterator for &'a SignalSet {
    type Item = Signal;
    type IntoIter = SignalSetIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Signal based timer
///
/// raises a `Signal(ALRM)` signal on expiry
pub struct Timer(libc::itimerval);

impl Default for Timer {
    fn default() -> Self {
        Self(libc::itimerval {
            it_interval: libc::timeval {
                tv_sec: 0,
                tv_usec: 0,
            },
            it_value: libc::timeval {
                tv_sec: 0,
                tv_usec: 0,
            },
        })
    }
}

impl Timer {
    /// Create a new timer
    pub fn new(duration: std::time::Duration, repeat: bool) -> Self {
        let mut ret = Timer::default();
        ret.set_duration(duration);
        if repeat {
            ret.set_interval(duration);
        }
        ret
    }

    /// Set timer duration
    pub fn set_duration(&mut self, duration: std::time::Duration) -> &mut Self {
        self.0.it_value.tv_sec = duration.as_secs() as i64;
        self.0.it_value.tv_usec = duration.subsec_micros() as i32;
        self
    }

    /// Set interval
    pub fn set_interval(&mut self, duration: std::time::Duration) -> &mut Self {
        self.0.it_interval.tv_sec = duration.as_secs() as i64;
        self.0.it_interval.tv_usec = duration.subsec_micros() as i32;
        self
    }

    /// Retrieve the timer duration
    pub fn duration(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.0.it_value.tv_sec as u64)
            + std::time::Duration::from_micros(self.0.it_value.tv_usec as u64)
    }

    /// Retrieve the timer interval
    pub fn interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.0.it_interval.tv_sec as u64)
            + std::time::Duration::from_micros(self.0.it_interval.tv_usec as u64)
    }

    /// Fetch current timer configuration from system
    ///
    /// - `duration` will be set to remaining time before next expiry
    /// - `interval` the timer interval
    pub fn fetch(&mut self) -> Result<()> {
        unsafe { libc_check(libc::getitimer(libc::ITIMER_REAL, &mut self.0)) }
    }

    /// Start the system timer
    pub fn start(&self) -> Result<()> {
        unsafe { libc_check(libc::setitimer(libc::ITIMER_REAL, &self.0, null_mut())) }
    }

    /// Stop the system timer
    ///
    /// Sets both interval and duration to zero on the system side
    pub fn stop(&self) -> Result<()> {
        let itimerval = libc::itimerval {
            it_interval: libc::timeval {
                tv_sec: 0,
                tv_usec: 0,
            },
            it_value: libc::timeval {
                tv_sec: 0,
                tv_usec: 0,
            },
        };
        unsafe { libc_check(libc::setitimer(libc::ITIMER_REAL, &itimerval, null_mut())) }
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use anyhow::Result;

    #[test]
    fn debug() {
        let sig = Signal(libc::SIGTERM);

        tracing::info!(?sig, sig2 = ?Signal(libc::SIGCHLD), "debug test");
    }

    #[test]
    fn timer() -> Result<()> {
        let sigset = SignalSet::default() + SIGALRM;
        sigset.block()?;

        let mut timer = Timer::default();
        timer
            .set_duration(Duration::from_millis(250))
            .set_interval(Duration::from_millis(250))
            .start()?;

        assert_eq!(SIGALRM, sigset.wait()?);
        assert_eq!(SIGALRM, sigset.wait()?);

        {
            let mut other = Timer::default();
            other.fetch()?;
            assert!(other.duration() < timer.duration());
            assert_eq!(other.interval(), timer.interval());
        }

        timer.stop()?;
        assert_eq!(timer.duration(), Duration::from_millis(250));
        assert_eq!(timer.interval(), Duration::from_millis(250));

        timer.fetch()?;
        assert_eq!(timer.duration(), Duration::from_millis(0));
        assert_eq!(timer.interval(), Duration::from_millis(0));

        Ok(())
    }

    #[test]
    fn signalset() {
        let sigset = SignalSet::default() + SIGALRM + SIGCHLD;

        for sig in &sigset {
            tracing::trace!(?sig, "signal in set");
        }
        let sigs: Vec<Signal> = sigset.iter().collect();
        assert_eq!(sigs.as_slice(), &[SIGALRM, SIGCHLD]);

        let sigset = sigset - SIGCHLD;
        assert_eq!(sigset.iter().count(), 1);
    }
}
