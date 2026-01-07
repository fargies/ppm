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
use std::{
    fmt::Debug,
    ops::Deref,
    ptr::{null, null_mut},
    sync::LazyLock,
};

#[cfg(target_os = "linux")]
mod posix;

#[cfg(target_os = "linux")]
pub use posix::Timer;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "macos")]
pub use macos::Timer;

/// POSIX Signal wrapper
#[derive(Clone, Copy, PartialEq)]
pub struct Signal(pub libc::c_int);

pub const SIGALRM: Signal = Signal(libc::SIGALRM);
pub const SIGCHLD: Signal = Signal(libc::SIGCHLD);
pub const SIGTERM: Signal = Signal(libc::SIGTERM);
#[allow(dead_code)]
pub const SIGSTOP: Signal = Signal(libc::SIGSTOP);
pub const SIGKILL: Signal = Signal(libc::SIGKILL);
pub const SIGINT: Signal = Signal(libc::SIGINT);

static FULL_SET: LazyLock<SignalSet> = LazyLock::new(|| {
    SignalSet(unsafe {
        let mut sigset: libc::sigset_t = std::mem::zeroed();
        libc_check(libc::sigfillset(&mut sigset)).unwrap();
        // remove signals that can't be controlled from the set
        libc_check(libc::sigdelset(&mut sigset, libc::SIGSTOP)).unwrap();
        libc_check(libc::sigdelset(&mut sigset, libc::SIGKILL)).unwrap();
        #[cfg(target_os = "macos")]
        libc_check(libc::sigdelset(&mut sigset, 32)).unwrap();
        sigset
    })
});

pub fn gettid() -> libc::pthread_t {
    unsafe { libc::pthread_self() }
}

pub fn getpid() -> libc::pid_t {
    unsafe { libc::getpid() }
}

impl Signal {
    #[tracing::instrument(level = "TRACE", err)]
    pub fn kill<S>(pid: libc::pid_t, signal: S) -> Result<()>
    where
        S: Into<Signal> + Debug,
    {
        unsafe { libc_check(libc::kill(pid, *signal.into())) }
    }

    #[tracing::instrument(level = "TRACE", err)]
    pub fn kill_thread<S>(tid: libc::pthread_t, signal: S) -> Result<()>
    where
        S: Into<Signal> + Debug,
    {
        unsafe { libc_check(libc::pthread_kill(tid, *signal.into())) }
    }

    #[tracing::instrument(level = "TRACE", err)]
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
            libc::SIGINT => f.write_str("SIGINT"),
            sig => write!(f, "SIG({})", sig),
        }
        // f.write_str(")")
    }
}

impl From<libc::c_int> for Signal {
    fn from(value: libc::c_int) -> Self {
        Signal(value)
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
            .field(&format_args!("{:X}", unsafe {
                *(&self.0 as *const _ as *const u32)
            }))
            .finish()
    }
}

impl SignalSet {
    /// Block signals in the set
    #[tracing::instrument(fields(pid= getpid(), tid = gettid()), level = "TRACE", err)]
    pub fn block(&self) -> Result<()> {
        unsafe { libc_check(libc::pthread_sigmask(libc::SIG_BLOCK, &self.0, null_mut())) }
    }

    /// Unblock signals in the set
    #[tracing::instrument(fields(pid= getpid(), tid = gettid()), level = "TRACE", err)]
    pub fn unblock(&self) -> Result<()> {
        unsafe {
            libc_check(libc::pthread_sigmask(
                libc::SIG_UNBLOCK,
                &self.0,
                null_mut(),
            ))
        }
    }

    /// Build a full-set
    pub fn full() -> Self {
        Self(FULL_SET.0)
    }

    /// Retrieve pending signals set
    #[tracing::instrument(level = "TRACE", ret)]
    pub fn pending() -> Self {
        Self(unsafe {
            let mut set: libc::sigset_t = std::mem::zeroed();
            libc_check(libc::sigpending(&mut set)).unwrap();
            set
        })
    }

    pub fn load() -> Self {
        Self(unsafe {
            let mut set: libc::sigset_t = std::mem::zeroed();
            libc_check(libc::pthread_sigmask(libc::SIG_UNBLOCK, null(), &mut set)).unwrap();
            set
        })
    }

    /// Build an empty set
    pub fn empty() -> Self {
        Self(unsafe {
            let mut set: libc::sigset_t = std::mem::zeroed();
            libc_check(libc::sigemptyset(&mut set)).unwrap();
            set
        })
    }

    /// Wait for a (blocked) signal in the set to raise
    #[tracing::instrument(level = "TRACE", ret)]
    pub fn wait(&self) -> Result<Signal> {
        unsafe {
            loop {
                let mut sig: libc::c_int = 0;
                libc_check(libc::sigwait(&self.0, &mut sig))?;
                let sig = Signal(sig);
                if self.contains(sig) {
                    return Ok(sig);
                }
            }
        }
    }

    /// Fills the set with blockable signals
    pub fn fill(&mut self) -> &mut Self {
        self.0 = FULL_SET.0;
        self
    }

    /// Test if signal is in the set
    pub fn contains<S>(&self, signal: S) -> bool
    where
        S: Into<Signal>,
    {
        unsafe { libc::sigismember(&self.0, signal.into().0) == 1 }
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
        Self::empty()
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
        let mut sig = Signal(0);
        for i in self.index..32 {
            sig.0 = i.into();
            if self.sigset.contains(sig) {
                self.index = i + 1;
                return Some(sig);
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

#[cfg(test)]
mod tests {
    use serial_test::serial;
    use std::time::Duration;

    use super::*;
    use anyhow::Result;

    #[ctor::ctor]
    fn prepare() {
        // rust test framewrok uses threads, the main process may handle signals
        (SignalSet::empty() + SIGALRM + SIGTERM + SIGCHLD).block();
    }

    #[test]
    fn debug() {
        let sig = Signal(libc::SIGTERM);

        tracing::info!(?sig, sig2 = ?Signal(libc::SIGCHLD), "debug test");
    }

    #[test]
    #[serial(waitpid)]
    /// Some signals may not be blocked depending on the platform
    ///
    /// Ensure we can block and unblock a full signalset
    fn full_set() -> Result<()> {
        let sigset = SignalSet::full();
        sigset.block()?;
        sigset.unblock()
    }

    extern "C" fn blocked_sighandler(sig: libc::c_int) {
        panic!("blocked signal caught: {}", sig);
    }

    #[test]
    #[serial(waitpid)]
    fn pending() -> Result<()> {
        let sigset = SignalSet::empty() + SIGALRM;
        sigset.block()?;
        for sig in &sigset {
            sig.set_handler(blocked_sighandler as usize)?;
        }

        unsafe {
            libc::pthread_kill(libc::pthread_self(), libc::SIGALRM);
        }
        // Signal::kill(unsafe { libc::getpid() }, SIGALRM)?;
        std::thread::sleep(Duration::from_millis(100));

        assert!(SignalSet::pending().contains(SIGALRM));
        assert_eq!(SIGALRM, sigset.wait()?);
        assert!(!SignalSet::pending().contains(SIGALRM));
        sigset.restore()
    }

    #[test]
    #[serial(waitpid)]
    fn timer() -> Result<()> {
        let sigset = SignalSet::empty() + SIGALRM;
        sigset.block()?;

        let mut timer = Timer::default();
        timer
            .set_duration(Duration::from_millis(25))
            .set_interval(Duration::from_millis(25))
            .start()?;

        std::thread::sleep(Duration::from_millis(30));
        tracing::trace!(pending= ?SignalSet::pending());
        assert!(SignalSet::pending().contains(SIGALRM));
        assert_eq!(SIGALRM, sigset.wait()?);
        assert_eq!(SIGALRM, sigset.wait()?);

        timer.stop()?;
        std::thread::sleep(Duration::from_millis(30));
        assert!(!SignalSet::pending().contains(SIGALRM));
        assert_eq!(timer.duration(), Duration::from_millis(25));
        assert_eq!(timer.interval(), Duration::from_millis(25));
        sigset.restore()
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
