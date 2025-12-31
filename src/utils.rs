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

mod serde_utils;
pub use serde_utils::{wrap_iterator, wrap_map_iterator, InnerRef};

pub mod signal;

pub fn terminate(pid: libc::pid_t, signal: libc::c_int, timeout: std::time::Duration) -> bool {
    unsafe {
        libc::kill(pid, signal);
    }

    let start = std::time::Instant::now();
    loop {
        if waitpid(pid).is_some() {
            return true;
        } else if start.elapsed() < timeout {
            std::thread::sleep(std::time::Duration::from_millis(10));
        } else {
            return false;
        }
    }
}

pub fn waitpid(pid: libc::pid_t) -> Option<(libc::pid_t, libc::c_int)> {
    let mut status: libc::c_int = 0;
    let ret = unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG | libc::WUNTRACED) };
    if ret > 0 {
        Some((ret, status))
    } else {
        None
    }
}
