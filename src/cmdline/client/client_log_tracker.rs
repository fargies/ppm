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

use anyhow::Result;
use std::{
    cell::Cell,
    fs::File,
    io::{self, stdout},
    os::fd::{AsRawFd, RawFd},
    path::PathBuf,
};

use crate::{
    cmdline::Action,
    utils::{
        OnDrop,
        signal::{SIGINT, SIGTERM, Signal},
    },
};
#[cfg(target_os = "linux")]
use inotify::{EventMask, Inotify, WatchMask};

use super::Client;

pub struct ClientLogTracker<'a> {
    service: String,
    client: &'a Client,
    file: File,
    filename: PathBuf,
}

thread_local! {
    static LOG_TRACKER_FD: Cell<Option<RawFd>> = const { Cell::new(None) };
}

extern "C" fn client_sighandler(sig: libc::c_int) {
    if let Some(fd) = LOG_TRACKER_FD.take() {
        unsafe {
            libc::close(fd);
        }
    }
    // not using tracing in signal handlers
    eprintln!("interrupted ({:?})", Signal(sig));
}

impl<'a> ClientLogTracker<'a> {
    pub fn new(service: String, client: &'a Client, file: File, filename: PathBuf) -> Self {
        Self {
            service,
            client,
            file,
            filename,
        }
    }

    #[cfg(target_os = "linux")]
    pub fn log(&mut self) -> Result<()> {
        let mut buf = vec![0; 1024];
        let mut ino = Inotify::init()?;
        LOG_TRACKER_FD.replace(Some(ino.as_raw_fd()));

        ino.watches()
            .add(&self.filename, WatchMask::MODIFY | WatchMask::CLOSE_WRITE)?;
        let mut refresh = false;

        let _drop_guard = OnDrop::new(|| {
            SIGTERM.restore().unwrap();
            SIGINT.restore().unwrap();
            LOG_TRACKER_FD.take();
        });
        SIGTERM.set_handler(client_sighandler)?;
        SIGINT.set_handler(client_sighandler)?;

        loop {
            let _span = tracing::info_span!(parent: None, "log_tracker").entered();
            io::copy(&mut self.file, &mut stdout())?;

            for event in match ino.read_events_blocking(&mut buf) {
                Ok(event) => event,
                Err(_) if LOG_TRACKER_FD.get().is_none() => return Ok(()),
                Err(err) => return Err(err.into()),
            } {
                refresh = event.mask.contains(EventMask::CLOSE_WRITE);
                tracing::trace!(?event, refresh, "event received");
            }
            if refresh {
                if let Some(new_file) = self
                    .client
                    .invoke::<Vec<PathBuf>>(&Action::ListLogFiles {
                        service: self.service.clone(),
                    })?
                    .last()
                    && new_file != &self.filename
                {
                    tracing::debug!(file = ?new_file, "new log-file detected");
                    refresh = false;
                    self.filename = new_file.clone();
                    self.file = File::open(new_file)?;

                    ino = Inotify::init()?;
                    let old = LOG_TRACKER_FD.replace(Some(ino.as_raw_fd()));
                    if old.is_none() {
                        /* signal occured whilst replacing file */
                        return Ok(());
                    }
                    ino.watches()
                        .add(&self.filename, WatchMask::MODIFY | WatchMask::CLOSE_WRITE)?;
                } else {
                    tracing::warn!("new log-file not detected");
                    std::thread::sleep(std::time::Duration::from_secs(3));
                }
            }
        }
    }

    #[cfg(not(target_os = "linux"))]
    pub fn log(&mut self) -> Result<()> {
        let _drop_guard = OnDrop::new(|| {
            SIGTERM.restore().unwrap();
            SIGINT.restore().unwrap();
            LOG_TRACKER_FD.take();
        });

        SIGTERM.set_handler(client_sighandler)?;
        SIGINT.set_handler(client_sighandler)?;
        LOG_TRACKER_FD.replace(Some(self.file.as_raw_fd()));

        loop {
            let _span = tracing::info_span!(parent: None, "log_tracker").entered();
            for _ in 0..5 {
                match io::copy(&mut self.file, &mut stdout()) {
                    Ok(_) => (),
                    Err(_) if LOG_TRACKER_FD.get().is_none() => return Ok(()),
                    Err(err) => return Err(err.into()),
                };
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
            if let Some(new_file) = self
                .client
                .invoke::<Vec<PathBuf>>(&Action::ListLogFiles {
                    service: self.service.clone(),
                })?
                .last()
                && new_file != &self.filename
            {
                self.filename = new_file.clone();
                tracing::trace!(file = ?new_file, "new log-file detected");
                self.file = File::open(new_file)?;
                let old = LOG_TRACKER_FD.replace(Some(self.file.as_raw_fd()));
                if old.is_none() {
                    /* signal occured whilst replacing file */
                    return Ok(());
                }
            }
        }
    }
}
