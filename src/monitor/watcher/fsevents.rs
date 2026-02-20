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

use std::{
    collections::HashMap,
    ffi::{CStr, c_double, c_void},
    fmt::Debug,
    path::Path,
    sync::Weak,
};

use anyhow::Result;

use dispatch2::{DispatchQueue, DispatchRetained, GlobalQueueIdentifier};
use objc2_core_foundation::{CFArray, CFString, kCFAllocatorDefault};
use objc2_core_services::{
    ConstFSEventStreamRef, FSEventStreamContext, FSEventStreamCreate, FSEventStreamFlushSync,
    FSEventStreamRef, FSEventStreamRelease, FSEventStreamSetDispatchQueue, FSEventStreamStart,
    FSEventStreamStop, kFSEventStreamCreateFlagFileEvents, kFSEventStreamCreateFlagWatchRoot,
    kFSEventStreamEventFlagItemCreated, kFSEventStreamEventFlagItemIsDir,
    kFSEventStreamEventFlagItemIsFile, kFSEventStreamEventFlagItemModified,
    kFSEventStreamEventFlagItemRemoved, kFSEventStreamEventFlagItemRenamed,
    kFSEventStreamEventIdSinceNow,
};

use crate::{
    monitor::Monitor,
    service::{ServiceId, Watch},
    utils::debug::DebugIter,
};

use super::WatcherTrait;

pub type Watcher = FSEventWatcher;

/// A [WatcherTrait] object that relies on MacOS FSEvents
pub struct FSEventWatcher {
    monitor: Weak<Monitor>,
    queue: DispatchRetained<DispatchQueue>,
    watchs: HashMap<ServiceId, WatchInfo>,
}

impl Debug for FSEventWatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Watcher")
            .field("services", &DebugIter::new(self.watchs.keys()))
            .finish()
    }
}

impl WatcherTrait for FSEventWatcher {
    fn new(monitor: Weak<Monitor>) -> Result<Self> {
        // using a dedicated queue will spawn a thread
        // using [DispatchQueue::main] will wait until someone calls [dispatch2::dispatch_main]
        // [FSEventStreamUnscheduleFromRunLoop] is deprecated and would work if we call [CRunLoop::run]
        Ok(Self {
            monitor,
            queue: DispatchQueue::global_queue(GlobalQueueIdentifier::QualityOfService(
                dispatch2::DispatchQoS::Default,
            )),
            watchs: Default::default(),
        })
    }

    fn add(&mut self, service_id: &ServiceId, watch: &Watch) -> Result<()> {
        self.watchs.insert(
            *service_id,
            WatchInfo::new(service_id, watch, self.queue.as_ref(), self.monitor.clone()),
        );
        Ok(())
    }

    fn remove(&mut self, service_id: &ServiceId) {
        self.watchs.remove(service_id);
    }

    fn has_watch(&self) -> bool {
        self.watchs.contains(service_id)
    }
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct FSEventFlags: u32 {
        const CREATED  = kFSEventStreamEventFlagItemCreated;
        const REMOVED  = kFSEventStreamEventFlagItemRemoved;
        const MODIFIED = kFSEventStreamEventFlagItemModified;
        const RENAMED  = kFSEventStreamEventFlagItemRenamed;
        const IS_FILE  = kFSEventStreamEventFlagItemIsFile;
        const IS_DIR   = kFSEventStreamEventFlagItemIsDir;
    }
}

struct WatchInfoData {
    service_id: ServiceId,
    monitor: Weak<Monitor>,
}

impl WatchInfoData {
    pub fn new(service_id: ServiceId, monitor: Weak<Monitor>) -> Box<Self> {
        Box::new(Self {
            service_id,
            monitor,
        })
    }

    pub fn process(&self, path: &str, flags: FSEventFlags) {
        let monitor = match self.monitor.upgrade() {
            Some(m) => m,
            None => {
                tracing::error!("monitor has been released");
                return;
            }
        };
        let service = match monitor.get(&self.service_id) {
            Some(service) => service,
            None => {
                tracing::error!(id = self.service_id, "unknown service");
                return;
            }
        };
        if let Some(watch) = service.watch.as_ref()
            && Path::new(path)
                .file_name()
                .is_some_and(|name| !watch.is_excluded(name.as_ref()))
        {
            tracing::info!(id=service.id, name=service.name, file=?path,
                event=?flags,
                "file event detected");
            monitor.on_watch_event(&service);
        } else {
            tracing::trace!(id=service.id, name=service.name, file=?path, event=?flags,
                "file event rejected")
        }
    }

    pub fn as_void(&self) -> *mut c_void {
        self as *const _ as *mut c_void
    }
}

struct WatchInfo {
    stream: FSEventStreamRef,
    // code is not dead, it's sent using raw-pointer to the callback
    #[allow(dead_code)]
    data: Box<WatchInfoData>,
}

impl WatchInfo {
    pub fn new(
        service_id: &ServiceId,
        watch: &Watch,
        queue: &DispatchQueue,
        monitor: Weak<Monitor>,
    ) -> Self {
        let paths = CFArray::from_retained_objects(
            &watch
                .paths
                .iter()
                .map(|path| {
                    tracing::trace!(?path, "adding watch");
                    CFString::from_str(path.as_os_str().to_str().expect("invalid path"))
                })
                .collect::<Vec<_>>(),
        );
        let data = WatchInfoData::new(*service_id, monitor);
        let mut context = FSEventStreamContext {
            version: 0,
            info: data.as_void(),
            retain: None,
            release: None,
            copyDescription: None,
        };
        unsafe {
            let stream = FSEventStreamCreate(
                kCFAllocatorDefault,
                Some(Self::on_events),
                &mut context,
                paths.as_ref(),
                kFSEventStreamEventIdSinceNow,
                0.2 as c_double,
                kFSEventStreamCreateFlagFileEvents | kFSEventStreamCreateFlagWatchRoot,
            );
            FSEventStreamSetDispatchQueue(stream, Some(queue));
            FSEventStreamStart(stream);
            Self { stream, data }
        }
    }

    extern "C-unwind" fn on_events(
        _stream_ref: ConstFSEventStreamRef,
        info: *mut c_void,
        num_events: usize,
        event_paths: std::ptr::NonNull<c_void>,
        event_flags: std::ptr::NonNull<u32>,
        _event_ids: std::ptr::NonNull<u64>,
    ) {
        let data = unsafe { &*(info as *const WatchInfoData) };
        let event_paths = unsafe {
            core::slice::from_raw_parts(event_paths.as_ptr() as *const *const i8, num_events)
        };
        let event_flags = unsafe { core::slice::from_raw_parts(event_flags.as_ptr(), num_events) };

        for (&path, &flags) in event_paths.iter().zip(event_flags) {
            match unsafe { CStr::from_ptr(path) }.to_str() {
                Ok(path) => data.process(path, FSEventFlags::from_bits_truncate(flags)),
                Err(err) => tracing::error!(?err, "invalid utf8 string for path"),
            }
        }
    }

    pub fn stop(&mut self) {
        if !self.stream.is_null() {
            unsafe {
                FSEventStreamFlushSync(self.stream);
                FSEventStreamStop(self.stream);
                FSEventStreamSetDispatchQueue(self.stream, None);
                FSEventStreamRelease(self.stream);
                self.stream = std::ptr::null_mut();
            }
        }
    }
}

impl Drop for WatchInfo {
    fn drop(&mut self) {
        self.stop();
    }
}

unsafe impl Send for WatchInfo {}
unsafe impl Sync for WatchInfo {}
