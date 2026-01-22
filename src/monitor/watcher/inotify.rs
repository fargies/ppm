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
use anyhow::{Context, Result, anyhow};
use dashmap::DashMap;
use libc::{POLLERR, POLLIN, nfds_t, poll, pollfd};
use std::{
    fmt::{self, Debug},
    fs::read_dir,
    io::{PipeReader, PipeWriter, Read, Write, pipe},
    os::fd::{AsRawFd, RawFd},
    path::Path,
    sync::{Arc, Mutex, Weak},
    thread::JoinHandle,
};

use super::Monitor;
use crate::{
    service::{ServiceId, Watch},
    utils::{debug::DebugIter, libc::check},
};

use crate::service::Service;

use super::*;
use anyhow::Result;
use inotify::{Inotify, WatchMask, Watches};

const WAKE_WORD: u8 = b'x';
const EXIT_WORD: u8 = b'q';

type WatchMap = Arc<DashMap<RawFd, Arc<WatchInfo>>>;

/// Inotify based directory watcher
pub struct Watcher {
    tx: PipeWriter,
    join_handle: Option<JoinHandle<()>>,
    watchs: WatchMap,
}

impl Watcher {
    /// Create a new [Watcher] object
    ///
    /// This does not register services
    pub fn new(monitor: &Arc<Monitor>) -> Result<Self> {
        {
            let (rx, tx) = pipe()?;
            let mut ret = Self {
                tx,
                join_handle: None,
                watchs: Arc::new(DashMap::new()),
            };

            let join_handle = {
                let mut ctx = WatcherThreadContext {
                    watchs: Arc::clone(&ret.watchs),
                    rx,
                    monitor: Arc::downgrade(monitor),
                    buffer: vec![0; 4096],
                };
                std::thread::spawn(move || {
                    ctx.run()
                        .inspect_err(|err| tracing::error!(?err, "watcher thread error"))
                        .unwrap_or_default()
                })
            };
            ret.join_handle = Some(join_handle);
            Ok(ret)
        }
    }

    pub fn wake(&mut self) {
        if let Err(err) = self.tx.write(&[WAKE_WORD]) {
            tracing::error!(?err, "failed to send wake-word");
        }
    }

    pub fn stop(&mut self) {
        if let Some(join_handle) = self.join_handle.take() {
            if let Err(err) = self.tx.write(&[EXIT_WORD]) {
                tracing::error!(?err, "failed to send exit-word");
            }
            if let Err(err) = join_handle.join() {
                tracing::error!(?err, "watcher thread join error");
            }
        }
    }

    pub fn add(&mut self, service_id: &ServiceId, watch: &Watch) -> Result<()> {
        let winfo = WatchInfo::new(service_id, watch)?;

        // remove duplicates
        self.watchs
            .retain(|_, value| &value.service_id != service_id);
        self.watchs.insert(winfo.as_raw_fd(), Arc::new(winfo));
        self.wake();

        Ok(())
    }

    pub fn remove(&mut self, service_id: &ServiceId) {
        self.watchs
            .retain(|_, value| &value.service_id != service_id);
        self.wake();
    }
}

struct WatcherThreadContext {
    watchs: WatchMap,
    rx: PipeReader,
    monitor: Weak<Monitor>,
    buffer: Vec<u8>,
}

impl WatcherThreadContext {
    fn make_pfds(&self) -> Vec<pollfd> {
        let mut pfds = Vec::with_capacity(self.watchs.len() + 1);
        for pfd in self.watchs.iter().map(|x| pollfd {
            fd: *x.key(),
            events: POLLIN | POLLERR,
            revents: 0,
        }) {
            pfds.push(pfd);
        }
        pfds.push(pollfd {
            fd: self.rx.as_raw_fd(),
            events: POLLIN | POLLERR,
            revents: 0,
        });
        pfds
    }

    fn monitor(&self) -> Result<Arc<Monitor>> {
        self.monitor
            .upgrade()
            .ok_or_else(|| anyhow!("monitor has been released"))
    }

    fn get_info(&self, fd: RawFd) -> Option<Arc<WatchInfo>> {
        self.watchs.get(&fd).map(|x| Arc::clone(&x))
    }

    pub fn run(&mut self) -> Result<()> {
        let mut pfds = self.make_pfds();
        let mut update_pfds = false;

        loop {
            let _span = tracing::info_span!(parent: None, "watcher").entered();

            if update_pfds {
                pfds = self.make_pfds();
                update_pfds = false;
            }

            let mut ret = unsafe { poll(pfds.as_mut_ptr(), pfds.len() as nfds_t, -1) };
            if ret < 0 {
                check(ret).context("failed to poll")?;
            }

            for pfd in pfds.iter().take(pfds.len() - 1) {
                if pfd.revents == 0 {
                    continue;
                }

                match self.get_info(pfd.fd) {
                    Some(info) => update_pfds = self.process(&info)?,
                    None => update_pfds = true,
                };

                ret -= 1;
                if ret <= 0 {
                    break;
                }
            }

            if pfds.last().unwrap().revents != 0 {
                let mut wake_word = [0u8; 1];
                if self.rx.read(&mut wake_word)? == 1 {
                    match wake_word[0] {
                        WAKE_WORD => {
                            update_pfds = true;
                            tracing::trace!("wake-word received");
                        }
                        EXIT_WORD => {
                            tracing::trace!("exit requested");
                            return Ok(());
                        }
                        wake_word => {
                            tracing::error!(wake_word, "unknown wake_word received")
                        }
                    }
                }
            }
        }
    }

    fn process(&mut self, info: &Arc<WatchInfo>) -> Result<bool> {
        let service = match self.monitor()?.get(&info.service_id) {
            Some(service) => service,
            None => return Ok(false),
        };

        info.process(&service, &mut self.buffer)
    }
}

impl Debug for Watcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        struct WatchWrapper<'a>(&'a WatchMap);

        impl Debug for WatchWrapper<'_> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                let mut f = f.debug_list();
                for it in self.0.iter() {
                    f.entry(&it.service_id);
                }
                f.finish()
            }
        }

        f.debug_struct("Watcher")
            .field(
                "services",
                &DebugIter::new(self.watchs.iter().map(|x| x.key)),
            )
            .finish()
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        self.stop();
    }
}

pub struct WatchInfo {
    pub service_id: ServiceId,
    inotify: Mutex<Inotify>,
}

impl WatchInfo {
    pub fn new(service_id: &ServiceId, watch: &Watch) -> Result<Self> {
        let inotify = Inotify::init()?;

        for path in watch.paths.iter() {
            if watch.is_excluded(path) {
                tracing::warn!(
                    ?path,
                    "configured path is excluded, add it to the `include` list"
                );
            } else {
                WatchInfo::register(&mut inotify.watches(), path, watch, 0);
            }
        }
        Ok(Self {
            service_id: *service_id,
            inotify: Mutex::new(inotify),
        })
    }

    fn register(watches: &mut Watches, path: &Path, watch: &Watch, level: usize) {
        if level >= watch.max_depth {
            tracing::error!(?path, level, "max watcher recursion level reached");
            return;
        }
        tracing::trace!(?path, "adding watch");

        if path.is_dir() {
            if let Err(err) = watches.add(
                path,
                WatchMask::CREATE
                    | WatchMask::DELETE
                    | WatchMask::MODIFY
                    | WatchMask::MOVED_TO
                    | WatchMask::MOVED_FROM,
            ) {
                tracing::error!(?err, ?path, "failed to watch dir");
            }

            match read_dir(path) {
                Ok(rd) => {
                    for file in rd.filter_map(|x| x.ok()) {
                        let path = file.path();
                        if path.is_dir() && !watch.is_excluded(&path) {
                            Watcher::register(watches, &path, watch, level + 1);
                        }
                    }
                }
                Err(err) => tracing::error!(?err, ?path, "failed to read dir"),
            }
        } else if path.is_file()
            && let Err(err) = watches.add(
                path,
                WatchMask::CREATE | WatchMask::DELETE | WatchMask::MODIFY | WatchMask::all(),
            )
        {
            tracing::error!(?err, ?path, "failed to watch file");
        }
    }

    pub fn process(&self, service: &Arc<Service>, buffer: &mut Vec<u8>) -> Result<bool> {
        for event in self
            .inotify
            .lock()
            .unwrap()
            .read_events(buffer.as_mut_slice())?
        {
            if let Some(name) = event.name {
                if service
                    .watch
                    .as_ref()
                    .is_some_and(|w| !w.is_excluded(Path::new(name)))
                {
                    tracing::info!(id=service.id, name=service.name, file=?name,
                        event=?DebugIter::new(event.mask.iter_names().map(|x| x.0)),
                        "file event detected");
                    // FIXME: should be throttled somehow ? use the scheduler ?
                    service.restart();
                } else {
                    tracing::trace!(id=service.id, name=service.name, file=?name,
                        event=?DebugIter::new(event.mask.iter_names().map(|x| x.0)),
                        "file event rejected")
                }
            }
        }

        Ok(true)
    }
}

impl AsRawFd for WatchInfo {
    fn as_raw_fd(&self) -> RawFd {
        self.inotify.lock().unwrap().as_raw_fd()
    }
}
