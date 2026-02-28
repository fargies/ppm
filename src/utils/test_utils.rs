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
** Author: Sylvain Fargier <fargier.sylvain@gmail.com>
*/

use super::{
    OnDrop,
    libc::getpid,
    signal::{SIGTERM, Signal},
};
use anyhow::Result;

#[tracing::instrument(level = "DEBUG", name = "wait_for", skip(fun))]
pub(crate) fn _wait_for<F, K>(mut fun: F, expiry: std::time::Duration) -> anyhow::Result<K>
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
                    tracing::trace!(attemps = count, "Ok({ret:?})");
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
    ($cond:expr $(,)?) => { $crate::utils::test_utils::_wait_for(|| {
        anyhow::ensure!($cond);
        return Ok(());
    }, std::time::Duration::from_secs(5)) };
    ($cond:expr, $dur:expr $(,)?) => { $crate::utils::test_utils::_wait_for(|| {
        anyhow::ensure!($cond);
        return Ok(());
    }, $dur) };
    ($cond:expr, $dur:expr, $msg:literal, $($arg:tt)* $(,)?) => { $crate::utils::test_utils::_wait_for(|| {
        anyhow::ensure!($cond, $msg, $($arg)*);
        return Ok(());
    }, $dur) };
    ($cond:expr, $msg:literal, $($arg:tt)* $(,)?) => { $crate::utils::test_utils::_wait_for(|| {
        anyhow::ensure!($cond, $msg, $($arg)*);
        return Ok(());
    }, std::time::Duration::from_secs(5)) };
}
#[cfg(test)]
pub(crate) use wait_for;

pub(crate) fn kill_on_drop<T>(
    join_handle: std::thread::JoinHandle<Result<T>>,
) -> OnDrop<impl FnOnce()> {
    OnDrop::new(move || {
        Signal::kill(getpid(), SIGTERM).unwrap();
        join_handle.join().unwrap().unwrap();
    })
}
