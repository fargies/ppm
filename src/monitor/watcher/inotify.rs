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
use std::{
    fmt::{self, Debug},
    fs::read_dir,
    os::fd::{AsRawFd, RawFd},
    path::Path,
    sync::{Arc, Mutex, Weak},
    thread::JoinHandle,
};

use super::{Monitor, WatcherTrait};
use crate::{
    service::{ServiceId, Watch},
    utils::{
        debug::DebugIter,
        poller::{Poller, PollerFds, PollerFlags, PollerWord, PollerWriter},
    },
};

use inotify::{Inotify, WatchMask, Watches};

type WatchMap = Arc<DashMap<RawFd, Arc<WatchInfo>>>;

pub type Watcher = InotifyWatcher;

/// Inotify based directory watcher
pub struct InotifyWatcher {
    poller: PollerWriter,
    join_handle: Option<JoinHandle<()>>,
    watchs: WatchMap,
}

impl InotifyWatcher {
    pub fn wake(&mut self) {
        self.poller.wake();
    }

    pub fn stop(&mut self) {
        if let Some(join_handle) = self.join_handle.take() {
            self.poller.exit();
            if let Err(err) = join_handle.join() {
                tracing::error!(?err, "watcher thread join error");
            }
        }
    }
}

impl WatcherTrait for InotifyWatcher {
    /// Create a new [Watcher] object
    ///
    /// This does not register services
    fn new(monitor: Weak<Monitor>) -> Result<Self> {
        {
            let (poller, poller_writer) = Poller::new();
            let mut ret = Self {
                poller: poller_writer,
                join_handle: None,
                watchs: Arc::new(DashMap::new()),
            };

            let join_handle = {
                let mut ctx = WatcherThreadContext::new(&ret, poller, monitor);
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

    fn add(&mut self, service_id: &ServiceId, watch: &Watch) -> Result<()> {
        let winfo = WatchInfo::new(service_id, watch)?;

        // remove duplicates
        self.watchs
            .retain(|_, value| &value.service_id != service_id);
        self.watchs.insert(winfo.as_raw_fd(), Arc::new(winfo));
        self.wake();

        Ok(())
    }

    fn remove(&mut self, service_id: &ServiceId) {
        self.watchs
            .retain(|_, value| &value.service_id != service_id);
        self.wake();
    }

    fn has_watch(&self, service_id: &ServiceId) -> bool {
        self.watchs
            .iter()
            .any(|value| &value.service_id == service_id)
    }
}

struct WatcherThreadContext {
    watchs: WatchMap,
    poller: Poller,
    monitor: Weak<Monitor>,
    buffer: Vec<u8>,
}

impl WatcherThreadContext {
    pub fn new(watcher: &Watcher, poller: Poller, monitor: Weak<Monitor>) -> Self {
        Self {
            watchs: Arc::clone(&watcher.watchs),
            poller,
            monitor,
            buffer: vec![0; 4096],
        }
    }

    fn prepare(&self, pfds: &mut PollerFds) {
        pfds.clear();

        for watch in self.watchs.iter() {
            pfds.push(
                watch.key(),
                PollerFlags::IN | PollerFlags::ERR | PollerFlags::NVAL,
            );
        }
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
        let mut pfds = PollerFds::with_capacity(self.watchs.len());
        let mut update_pfds = true;

        loop {
            let _span = tracing::info_span!(parent: None, "watcher").entered();

            if update_pfds {
                self.prepare(&mut pfds);
                update_pfds = false;
            }

            let wake_word = self.poller.poll(&mut pfds).context("failed to poll")?;
            tracing::trace!(?wake_word, events = ?DebugIter::new(pfds.iter()), "watcher awaken");

            for (fd, flags) in pfds.iter() {
                if flags.contains(PollerFlags::IN) {
                    match self.get_info(fd) {
                        Some(info) => {
                            update_pfds = info.process(&self.monitor()?, &mut self.buffer)?
                        }
                        None => update_pfds = true,
                    };
                }
            }

            match wake_word {
                Some(PollerWord::Wake) => {
                    update_pfds = true;
                    tracing::trace!("wake-word received");
                }
                Some(PollerWord::Exit) => {
                    tracing::trace!("exit requested");
                    return Ok(());
                }
                Some(PollerWord::Custom(wake_word)) => {
                    tracing::error!(wake_word, "unknown wake_word received")
                }
                None => (),
            }
        }
    }
}

impl Debug for InotifyWatcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Watcher")
            .field(
                "services",
                &DebugIter::new(self.watchs.iter().map(|x| *x.key())),
            )
            .finish()
    }
}

impl Drop for InotifyWatcher {
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
        tracing::trace!(fd = inotify.as_raw_fd(), "new inotify watch");

        for path in watch.paths.iter() {
            /* configured paths are never excluded */
            WatchInfo::register(&mut inotify.watches(), path, watch, 0);
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
                            WatchInfo::register(watches, &path, watch, level + 1);
                        }
                    }
                }
                Err(err) => tracing::error!(?err, ?path, "failed to read dir"),
            }
        } else if path.is_file()
            && let Err(err) = watches.add(
                path,
                WatchMask::MODIFY | WatchMask::DELETE_SELF | WatchMask::MOVE_SELF,
            )
        {
            tracing::error!(?err, ?path, "failed to watch file");
        }
    }

    pub fn process(&self, monitor: &Arc<Monitor>, buffer: &mut Vec<u8>) -> Result<bool> {
        let service = match monitor.get(&self.service_id) {
            Some(service) => service,
            None => return Ok(false),
        };

        for event in self
            .inotify
            .lock()
            .unwrap()
            .read_events(buffer.as_mut_slice())
            .context("inotify error")?
        {
            match event.name {
                Some(name) => {
                    if service
                        .watch
                        .as_ref()
                        .is_some_and(|w| !w.is_excluded(Path::new(name)))
                    {
                        tracing::info!(id = service.id, name = service.name, file = ?name,
                            event = ?DebugIter::new(event.mask.iter_names().map(|x| x.0)),
                            "dir event detected");
                        monitor.on_watch_event(&service);
                    } else {
                        tracing::trace!(id = service.id, name = service.name, file = ?name,
                            event = ?DebugIter::new(event.mask.iter_names().map(|x| x.0)),
                            "dir event rejected")
                    }
                }
                None => {
                    tracing::info!(id = service.id, name = service.name,
                        event = ?DebugIter::new(event.mask.iter_names().map(|x| x.0)),
                        "file event detected");
                    monitor.on_watch_event(&service);
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

impl Drop for WatchInfo {
    fn drop(&mut self) {
        tracing::trace!(
            fd = self.inotify.lock().unwrap().as_raw_fd(),
            "closing inotify watch"
        );
    }
}
