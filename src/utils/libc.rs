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
** Created on: 2026-01-09T16:02:25
** Author: Sylvain Fargier <fargier.sylvain@gmail.com>
*/

use anyhow::Result;
use libc::{c_int, pid_t};

/// Set session-id
///
/// Returns the new session-id
pub fn setsid() -> Result<pid_t> {
    let ret = unsafe { libc::setsid() };

    if ret < 0 {
        Err(std::io::Error::last_os_error().into())
    } else {
        Ok(ret)
    }
}

#[cfg(target_os = "linux")]
pub fn set_child_subreaper(pid: pid_t) -> Result<()> {
    check(unsafe { libc::prctl(libc::PR_SET_CHILD_SUBREAPER, pid) })
}

pub fn gettid() -> libc::pthread_t {
    unsafe { libc::pthread_self() }
}

pub fn getpid() -> pid_t {
    unsafe { libc::getpid() }
}

/// Invoke waitpid in non-blocking mode
pub fn waitpid(pid: pid_t, blocking: bool) -> Option<(pid_t, c_int)> {
    let mut status: c_int = 0;
    let ret = unsafe {
        libc::waitpid(
            pid,
            &mut status,
            if blocking { 0 } else { libc::WNOHANG } | libc::WUNTRACED, /* [libc::WUNTRACED] is required to detect SIGSTOP */
        )
    };
    if ret > 0 { Some((ret, status)) } else { None }
}

/// assert for libc functions
pub fn check(res: c_int) -> Result<()> {
    if res != 0 {
        let err = std::io::Error::last_os_error();
        tracing::trace_span!("libc_check", ?err);
        Err(err.into())
    } else {
        Ok(())
    }
}
