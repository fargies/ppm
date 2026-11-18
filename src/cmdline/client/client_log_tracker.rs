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

use anyhow::{Context, Result};
use std::{
    cell::Cell,
    fs::File,
    io::{self, stdout},
    path::PathBuf,
    time::Duration,
};

use crate::{
    cmdline::Action,
    utils::{
        OnDrop,
        poller::{Poller, PollerFlags, PollerWriter},
        signal::{SIGINT, SIGTERM, Signal},
    },
};
#[cfg(target_os = "linux")]
use inotify::{EventMask, Inotify, WatchMask};

use super::Client;

pub struct ClientLogTracker<'a> {
    service: String,
    client: &'a Client,
    file: Option<File>,
    filename: PathBuf,
}

thread_local! {
    static LOG_TRACKER_FD: Cell<Option<PollerWriter>> = const { Cell::new(None) };
}

extern "C" fn client_sighandler(sig: libc::c_int) {
    if let Some(mut writer) = LOG_TRACKER_FD.take() {
        writer.exit();
    }
    // not using tracing in signal handlers
    eprintln!("interrupted ({:?})", Signal(sig));
}

impl<'a> ClientLogTracker<'a> {
    pub fn new(service: String, client: &'a Client, file: File, filename: PathBuf) -> Self {
        Self {
            service,
            client,
            file: Some(file),
            filename,
        }
    }

    #[cfg(target_os = "linux")]
    pub fn log(&mut self) -> Result<()> {
        let mut buf = vec![0; 1024];
        let mut ino = Inotify::init()?;

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

        let (mut poller, writer) = Poller::with_pipe();
        let mut pfds = poller.make_pfds(2);
        pfds.push(&stdout(), PollerFlags::HUP | PollerFlags::ERR);
        pfds.push(&ino, PollerFlags::IN | PollerFlags::ERR);
        LOG_TRACKER_FD.replace(Some(writer));

        loop {
            let _span = tracing::info_span!(parent: None, "log_tracker").entered();
            io::copy(
                &mut self.file.as_mut().context("already closed")?,
                &mut stdout(),
            )?;

            let timeout = if refresh {
                Some(Duration::from_secs(3))
            } else {
                None
            };
            match poller.poll(&mut pfds, timeout) {
                Ok(Some(_)) => return Ok(()),
                Ok(None) => (),
                Err(err) if err.kind() == io::ErrorKind::Interrupted => return Ok(()),
                Err(err) => return Err(err.into()),
            };
            for (_, flags) in pfds.iter() {
                if flags.has_error() {
                    tracing::info!(flags = ?flags, "output closed");
                    return Ok(());
                }
            }

            for event in ino.read_events(&mut buf)? {
                refresh = event.mask.contains(EventMask::CLOSE_WRITE);
                tracing::trace!(?event, refresh, "event received");
            }
            if refresh {
                loop {
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

                        ino = Inotify::init()?;
                        ino.watches()
                            .add(new_file, WatchMask::MODIFY | WatchMask::CLOSE_WRITE)?;
                        let file = File::open(new_file)?;

                        self.file = Some(file);
                        self.filename = new_file.clone();
                        pfds.pop();
                        pfds.push(&ino, PollerFlags::IN | PollerFlags::ERR);
                        break;
                    } else {
                        tracing::warn!("new log-file not detected");
                    }
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

        /* this is used mainly to detect output closure */
        let (mut poller, writer) = Poller::with_pipe();
        let mut pfds = poller.make_pfds(1);
        pfds.push(&stdout(), PollerFlags::HUP | PollerFlags::ERR);
        LOG_TRACKER_FD.replace(Some(writer));

        loop {
            let _span = tracing::info_span!(parent: None, "log_tracker").entered();

            /* poll file for new-data every sec for 5sec */
            for _ in 0..5 {
                if let size = io::copy(
                    &mut self.file.as_mut().context("already closed")?,
                    &mut stdout(),
                )? && size != 0
                {
                    tracing::trace!(size, "bytes copied");
                }
                /* using poll to sleep, monitoring output state */
                match poller.poll(&mut pfds, Some(Duration::from_secs(1))) {
                    Ok(Some(_)) => return Ok(()),
                    Ok(None) => (),
                    Err(err) if err.kind() == io::ErrorKind::Interrupted => return Ok(()),
                    Err(err) => return Err(err.into()),
                };
                for (_, flags) in pfds.iter() {
                    if flags.has_error() {
                        tracing::info!(flags = ?flags, "output closed");
                        return Ok(());
                    }
                }
            }

            /* check if file was rotated */
            if let Some(new_file) = self
                .client
                .invoke::<Vec<PathBuf>>(&Action::ListLogFiles {
                    service: self.service.clone(),
                })?
                .last()
                && new_file != &self.filename
            {
                tracing::trace!(file = ?new_file, "new log-file detected");
                let file = File::open(new_file)?;

                self.file = Some(file);
                self.filename = new_file.clone();
            }
        }
    }
}
