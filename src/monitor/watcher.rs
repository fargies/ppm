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

#[cfg(target_os = "linux")]
#[path = "watcher/linux.rs"]
mod private;

#[cfg(any(
    target_os = "macos",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd"
))]
#[path = "watcher/unix.rs"]
mod private;

use private::WatchInfo;

const WAKE_WORD: u8 = b'x';
const EXIT_WORD: u8 = b'q';

type WatchMap = Arc<DashMap<RawFd, Arc<WatchInfo>>>;

/// Directory watcher
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
            .field("services", &WatchWrapper(&self.watchs))
            .finish()
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use std::{fs::File, time::Duration};

    use crate::{
        service::{Command, Service},
        utils::{
            MkTemp,
            libc::getpid,
            signal::{self, Signal},
        },
    };
    use serde_yaml_ng as yaml;
    use serial_test::serial;

    use super::*;

    #[test]
    #[serial(waitpid)]
    fn watch() -> Result<()> {
        let temp = MkTemp::dir("ppm-watch")?;

        let mon = Arc::new(Monitor::default());
        let service = {
            let mut srv = Service::new("test", Command::new("sleep", ["300"]));
            srv.watch = Some(yaml::from_str(
                format!("\"{}\"", temp.as_ref().to_str().unwrap()).as_str(),
            )?);
            srv.start();
            mon.insert(srv)
        };

        let join_handle = {
            /* Monitor is handling dead processes */
            let mon = Arc::clone(&mon);
            std::thread::spawn(move || mon.run())
        };

        std::thread::sleep(Duration::from_millis(100));
        tracing::trace!("creating test file");
        File::create(temp.as_ref().join("test_file"))?;
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(service.info().restarts, 2);

        Signal::kill(getpid(), signal::SIGTERM)?;
        join_handle.join().unwrap()?;
        Ok(())
    }
}
