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

use crate::service::Service;

use super::*;
use anyhow::Result;
use inotify::{Inotify, WatchMask, Watches};

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
                            WatchInfo::register(watches, &path, watch, level + 1);
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
