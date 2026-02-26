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

/* this binary uses only part of the utils */
#![allow(unused_imports)]
#![allow(dead_code)]

use anyhow::{Result, anyhow};
use std::{
    env,
    os::{
        fd::{FromRawFd, RawFd},
        unix::process::CommandExt,
    },
    process::{self, Stdio},
};

mod monitor;
mod service;
mod utils;

#[cfg(target_os = "linux")]
use crate::utils::signal::{SIGTERM, Signal};
use crate::utils::{
    libc::{Fcntl, getpid},
    signal::SignalSet,
    tracing_utils::tracing_init,
};

fn prepare_stdio(fd: RawFd) -> Stdio {
    match fd.get_flags().is_ok() {
        true => unsafe { Stdio::from_raw_fd(fd) },
        false => {
            tracing::debug!(fd, "fd not opened, forwarding to parent");
            Stdio::inherit()
        }
    }
}

/// This is an intermediate binary to properly
/// restore signals and set a couple flags on the new process.
///
/// It is safer to achieve this in a separate binary than
/// in-between fork/exec on the daemon process
/// (especially since it is multi-threaded).
///
fn main() -> Result<()> {
    tracing_init(std::io::stdout, Some("info"))?;
    let _span = tracing::info_span!(parent: None, "launcher",  pid = getpid()).entered();

    let mut args = env::args();
    args.next();
    let bin = args.next().ok_or(anyhow!("no exe to start"))?;

    SignalSet::full()
        .restore()
        .expect("failed to restore default signal handlers");

    #[cfg(target_os = "linux")]
    if let Err(err) = Signal::set_pdeath_sig(SIGTERM) {
        tracing::error!(?err, "failed to set pdeath signal");
    }

    tracing::trace!(bin, "starting process");
    let mut cmd = process::Command::new(bin);
    cmd.args(args)
        .stdin(process::Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    drop(_span);
    Err(anyhow!(cmd.exec()))
}
