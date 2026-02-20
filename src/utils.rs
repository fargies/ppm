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
** Created on: 2025-12-22T22:55:22
** Author: Sylvain Fargier <fargier.sylvain@gmail.com>
*/

use std::{
    io::IsTerminal,
    sync::{Arc, atomic::AtomicBool},
};

mod serde_utils;
pub use serde_utils::{InnerRef, LoadFromFile, wrap_map_iterator};

pub mod poller;

pub mod signal;

pub mod libc;

pub mod serializers;

mod lazy_bool;
pub use lazy_bool::LazyBool;

mod globset;
pub use globset::GlobSet;

pub mod debug;

mod buffer;
pub use buffer::Buffer;

#[cfg(test)]
mod mktemp;
#[cfg(test)]
pub use mktemp::MkTemp;

pub static IS_OUT_COLORED: LazyBool = LazyBool::new(|| {
    AtomicBool::new(
        std::io::stdout().is_terminal() && !std::env::var("NO_COLOR").is_ok_and(|v| !v.is_empty()),
    )
});

pub struct OnDrop<T>(Option<T>)
where
    T: FnOnce();

impl<T> OnDrop<T>
where
    T: FnOnce(),
{
    pub fn new(fun: T) -> Self {
        Self(Some(fun))
    }
}

impl<T> Drop for OnDrop<T>
where
    T: FnOnce(),
{
    fn drop(&mut self) {
        if let Some(callback) = self.0.take() {
            callback()
        }
    }
}

/// Convenience trait to pass `Arc<T>|T|&Arc<T>` as an argument
pub trait IntoArc<T> {
    fn into_arc(self) -> Arc<T>;
}

impl<T> IntoArc<T> for Arc<T> {
    fn into_arc(self) -> Arc<T> {
        self
    }
}

impl<T> IntoArc<T> for &Arc<T> {
    fn into_arc(self) -> Arc<T> {
        Arc::clone(self)
    }
}

impl<T> IntoArc<T> for T {
    fn into_arc(self) -> Arc<T> {
        Arc::new(self)
    }
}

#[cfg(test)]
#[tracing::instrument(level = "TRACE", name = "wait_for", skip(fun))]
pub fn _wait_for<F, K>(mut fun: F, expiry: std::time::Duration) -> anyhow::Result<K>
where
    F: FnMut() -> anyhow::Result<K>,
    K: std::fmt::Debug,
{
    let start = std::time::Instant::now();
    let mut count = 0;
    while start.elapsed() <= expiry {
        match fun() {
            Ok(ret) => {
                if count != 0 {
                    tracing::trace!(attemps = count);
                    // tracing::trace!("test succeeded after {count} attempt(s)");
                }
                return Ok(ret);
            }
            Err(err) => {
                if count == 0 {
                    tracing::trace!(?err, "test failed, trying again in 10ms");
                }
                count += 1;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    fun().inspect_err(|err| tracing::trace!(?err, "test failed"))
}

/// Test macro to poll a lambda until it validates
///
/// # Details
/// It'll return the last error on expiry.
///
/// Default timeout: 5 seconds.\
/// Polling interval: 10ms.
///
/// # Usage
/// ```rust
/// # use std::time::Duration;
/// let value = true;
/// wait_for!(value == true).expect("failed to check value");
/// wait_for!(value == true, "value failed to validate: {}", value)?;
/// wait_for!(value == true, Duration::from_secs(1), "value: {}", value).expect("failed to check value");
/// ```
#[cfg(test)]
macro_rules! wait_for {
    ($cond:expr $(,)?) => { $crate::utils::_wait_for(|| {
        anyhow::ensure!($cond);
        return Ok(());
    }, std::time::Duration::from_secs(5)) };
    ($cond:expr, $dur:expr $(,)?) => { $crate::utils::_wait_for(|| {
        anyhow::ensure!($cond);
        return Ok(());
    }, $dur) };
    ($cond:expr, $dur:expr, $msg:literal, $($arg:tt)* $(,)?) => { $crate::utils::_wait_for(|| {
        anyhow::ensure!($cond, $msg, $($arg)*);
        return Ok(());
    }, $dur) };
    ($cond:expr, $msg:literal, $($arg:tt)* $(,)?) => { $crate::utils::_wait_for(|| {
        anyhow::ensure!($cond, $msg, $($arg)*);
        return Ok(());
    }, std::time::Duration::from_secs(5)) };
}
#[cfg(test)]
pub(crate) use wait_for;
