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
use serde::{Deserialize, Serialize};

use std::{
    collections::{HashMap, VecDeque},
    fmt::Debug,
    os::fd::{AsRawFd, RawFd},
    path::PathBuf,
    process::Stdio,
    sync::{Arc, Mutex},
    thread::JoinHandle,
};

use dashmap::DashMap;

use crate::{
    service::{Service, ServiceId},
    utils::{
        Buffer,
        debug::DebugIter,
        poller::{Poller, PollerFds, PollerFlags, PollerWord, PollerWriter},
    },
};

mod logpump;
use logpump::LogPump;

mod logfile;
use logfile::LogFile;

const LOGGER_DEFAULT_PATH: &str = "/var/log/";

type LogMap = Arc<DashMap<ServiceId, LogPump>>;

#[derive(Serialize, Deserialize)]
#[serde(from = "LoggerOptions")]
pub struct Logger {
    path: Arc<PathBuf>,
    #[serde(skip)]
    logs: LogMap,
    #[serde(skip)]
    poller: Mutex<PollerWriter>,
    #[serde(skip)]
    join_handle: Option<JoinHandle<()>>,
}

#[derive(Deserialize)]
#[serde(default)]
pub struct LoggerOptions {
    path: PathBuf,
}

impl Default for LoggerOptions {
    fn default() -> Self {
        Self {
            path: LOGGER_DEFAULT_PATH.into(),
        }
    }
}

impl From<LoggerOptions> for Logger {
    fn from(value: LoggerOptions) -> Self {
        Logger::new(value.path)
    }
}

impl Debug for Logger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Logger").field("path", &self.path).finish()
    }
}

impl Logger {
    pub fn new<T>(log_dir: T) -> Self
    where
        T: Into<PathBuf>,
    {
        let (poller, tx) = Poller::new();
        let mut ret = Self {
            path: Arc::new(log_dir.into()),
            logs: Default::default(),
            poller: Mutex::new(tx),
            join_handle: None,
        };

        let join_handle = {
            let mut ctx = LoggerThreadContext::new(poller, Arc::clone(&ret.logs));
            std::thread::spawn(move || {
                ctx.run()
                    .inspect_err(|err| tracing::error!(?err, "logger thread error"))
                    .unwrap_or_default()
            })
        };
        ret.join_handle = Some(join_handle);
        ret
    }

    pub fn stop(&mut self) {
        if let Some(join_handle) = self.join_handle.take() {
            self.poller.lock().unwrap().exit();
            if let Err(err) = join_handle.join() {
                tracing::error!(?err, "watcher thread join error");
            }
        }
    }

    pub fn make_pipe(&self, service: &Service) -> Result<(Stdio, Stdio)> {
        let mut pump = match self.logs.remove(&service.id) {
            Some((_, pump)) => pump,
            None => LogPump::from(LogFile::new(&self.path, &service.name)),
        };
        pump.input.clear();
        pump.make_input().inspect(|_| {
            self.logs.insert(service.id, pump);
            self.wake();
        })
    }

    pub fn wake(&self) {
        self.poller.lock().unwrap().wake()
    }

    pub fn list_files(&self, service: ServiceId) -> Vec<PathBuf> {
        match self.logs.get(&service) {
            Some(log) => log.output.list_files(),
            None => Vec::new(),
        }
    }
}

impl Drop for Logger {
    fn drop(&mut self) {
        self.stop();
    }
}

struct LoggerThreadContext {
    poller: Poller,
    logs: LogMap,
    buffers: VecDeque<Buffer>,
}

impl LoggerThreadContext {
    pub fn new(poller: Poller, logs: LogMap) -> Self {
        Self {
            poller,
            logs,
            buffers: VecDeque::with_capacity(3),
        }
    }

    fn prepare(&self, pfds: &mut PollerFds, pfds_map: &mut HashMap<RawFd, ServiceId>) {
        pfds_map.clear();
        pfds.clear();

        for it in self.logs.iter() {
            if it.has_buffer()
                && let Some(fd) = it.output.as_raw_fd()
            {
                pfds_map.insert(fd, *it.key());
                pfds.push(&fd, PollerFlags::OUT | PollerFlags::ERR);
            } else {
                for f in it.input.iter() {
                    let fd = f.as_raw_fd();
                    pfds_map.insert(fd, *it.key());
                    pfds.push(&fd, PollerFlags::IN | PollerFlags::ERR);
                }
            }
        }
    }

    pub fn run(&mut self) -> Result<()> {
        let logs = Arc::clone(&self.logs);
        let mut pfds = PollerFds::with_capacity(logs.len() * 3);
        let mut pfds_map = HashMap::with_capacity(logs.len() * 3);
        loop {
            let _span = tracing::info_span!(parent: None, "logger").entered();

            self.prepare(&mut pfds, &mut pfds_map);

            let wake_word = self.poller.poll(&mut pfds).context("failed to poll")?;
            tracing::trace!(?wake_word, events = ?DebugIter::new(pfds.iter()), "logger awaken");

            for (fd, flags) in pfds.iter() {
                let service_id = match pfds_map.get(&fd) {
                    Some(service_id) => service_id,
                    None => {
                        tracing::trace!(fd, "no service bound to fd");
                        continue;
                    }
                };
                let mut pump = match logs.get_mut(service_id) {
                    Some(pump) => pump,
                    None => {
                        tracing::trace!(service_id, "no log-pump for service");
                        continue;
                    }
                };

                if let Some(buffer) = if flags.contains(PollerFlags::IN) {
                    pump.on_input_ready(fd, self.take_buffer())
                } else if !(flags & (PollerFlags::HUP | PollerFlags::ERR)).is_empty() {
                    pump.on_error(fd)
                } else if flags.contains(PollerFlags::OUT) {
                    pump.on_output_ready(fd)
                } else {
                    None
                } {
                    self.buffers.push_back(buffer);
                }
            }

            match wake_word {
                Some(PollerWord::Wake) => tracing::trace!("wake-word received"),
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

    fn take_buffer(&mut self) -> Buffer {
        self.buffers.pop_front().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        service::{Command, Service},
        utils::{MkTemp, libc::waitpid},
    };
    use anyhow::Result;
    use serial_test::serial;

    #[test]
    #[serial(waitpid)]
    fn logger() -> Result<()> {
        let temp_dir = MkTemp::dir("logger")?;
        let logger = Logger::new(temp_dir.as_ref());

        let srv = Service::new("test", Command::new("echo", ["world"]));

        srv.restart(&logger);
        waitpid(srv.info().pid.unwrap(), true).expect("failed to wait for srv");

        let files = logger.list_files(srv.id);
        assert_eq!(1, files.len());
        assert_eq!(6, files.first().unwrap().metadata()?.len());

        srv.restart(&logger);
        waitpid(srv.info().pid.unwrap(), true).expect("failed to wait for srv");

        let files = logger.list_files(srv.id);
        assert_eq!(1, files.len());
        assert_eq!(12, files.first().unwrap().metadata()?.len());

        Ok(())
    }
}
