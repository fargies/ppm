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
** Created on: 2025-12-22T15:40:32
** Author: Sylvain Fargier <fargier.sylvain@gmail.com>
*/

use crate::utils;
use crate::utils::signal::SignalSet;
use serde::{Deserialize, Serialize};
use std::os::unix::process::CommandExt;
use std::process;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

mod command;
pub use command::Command;

mod info;
pub use info::Info;

mod stats;
pub use stats::Stats;

mod status;
pub use status::Status;

static S_ID: AtomicUsize = AtomicUsize::new(0);

pub type ServiceId = usize;

#[derive(Debug, Serialize, Deserialize)]
pub struct Service {
    pub id: ServiceId,
    pub name: String,
    pub command: Command,
    #[serde(rename = "info")]
    _info: Mutex<Arc<Info>>,
    #[serde(rename = "stats")]
    _stats: Mutex<Arc<Stats>>,
}

impl Service {
    pub fn new<T>(name: T, command: Command) -> Self
    where
        T: ToString,
    {
        Self {
            id: S_ID.fetch_add(1, Ordering::Relaxed),
            name: name.to_string(),
            command,
            _info: Mutex::new(Arc::new(Info::default())),
            _stats: Mutex::new(Arc::new(Stats::default())),
        }
    }

    #[tracing::instrument(fields(name=self.name, id=self.id), skip(self))]
    pub fn start(&self) {
        self.restart()
    }

    #[tracing::instrument(fields(name=self.name, id=self.id), skip(self))]
    pub fn restart(&self) {
        if self.info().pid.is_some() {
            self.stop();
        }

        match unsafe { libc::fork() } {
            x if x < 0 => {
                tracing::error!("failed to fork");
            }
            0 => {
                SignalSet::default()
                    .fill()
                    .restore()
                    .expect("failed to restore default signal handlers");
                let mut cmd = process::Command::new(self.command.command.as_str());
                cmd.args(&self.command.args)
                    .stdin(process::Stdio::null())
                    .stdout(process::Stdio::inherit())
                    .stderr(process::Stdio::inherit());

                match self.command.env.as_ref() {
                    Some(env) => cmd.envs(env),
                    None => cmd.env_clear(),
                };
                let err = cmd.exec();
                tracing::error!("failed to spawn process: {}", err);
            }
            pid => {
                let mut guard = self._info.lock().unwrap();
                let info = Arc::make_mut(&mut guard);
                info.active = true;
                info.set_running(pid);
            }
        }
    }

    /// Stop the process
    #[tracing::instrument(fields(name=self.name, id=self.id), skip(self))]
    pub fn stop(&self) {
        if let Some(pid) = self.info().pid {
            if utils::waitpid(pid).is_some() {
                tracing::info!(pid = pid, "process (already) terminated");
            } else if utils::terminate(pid, libc::SIGTERM, std::time::Duration::from_secs(5)) {
                tracing::info!(pid = pid, "process terminated");
            } else if utils::terminate(pid, libc::SIGKILL, std::time::Duration::from_secs(10)) {
                tracing::info!(pid = pid, "process killed");
            } else {
                tracing::error!("failed to kill process");
            }
            let mut guard = self._info.lock().unwrap();
            let info = Arc::make_mut(&mut guard);
            info.active = false;
            info.set_finished();
        }
    }

    #[tracing::instrument(fields(name=self.name, id=self.id), skip(self))]
    pub fn set_crashed(&self) {
        let mut guard = self._info.lock().unwrap();
        Arc::make_mut(&mut guard).set_crashed();
    }

    #[tracing::instrument(fields(name=self.name, id=self.id), skip(self))]
    pub fn set_finished(&self) {
        let mut guard = self._info.lock().unwrap();
        Arc::make_mut(&mut guard).set_finished();
    }

    #[tracing::instrument(fields(name=self.name, id=self.id), skip(self))]
    pub fn set_stopped(&self) {
        let mut guard = self._info.lock().unwrap();
        Arc::make_mut(&mut guard).set_stopped();
    }

    #[tracing::instrument(fields(name=self.name, id=self.id), skip(self))]
    pub fn set_running(&self, pid: libc::pid_t) {
        let mut guard = self._info.lock().unwrap();
        Arc::make_mut(&mut guard).set_running(pid);
    }

    pub fn info(&self) -> Arc<Info> {
        Arc::clone(&self._info.lock().unwrap())
    }

    pub fn stats(&self) -> Arc<Stats> {
        Arc::clone(&self._stats.lock().unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn() {
        let mut srv = Service::new("test", Command::new("ls", ["-la"]));
        srv.start();
        assert!(srv.info().pid.is_some_and(|pid| pid > 0));
        assert_eq!(srv.info().status, Status::Running);

        // wait for command to terminate
        std::thread::sleep(std::time::Duration::from_millis(100));
        srv.stop();
        assert_eq!(srv.info().pid, None);
        assert_eq!(srv.info().status, Status::Stopped);
    }

    #[test]
    fn stop() {
        let srv = Service::new("test", Command::new("sh", ["-c", "sleep 300"]));
        srv.start();
        assert!(srv.info().pid.is_some_and(|pid| pid > 0));
        assert_eq!(srv.info().status, Status::Running);

        srv.stop();
        assert_eq!(srv.info().pid, None);
        assert_eq!(srv.info().status, Status::Stopped);
    }

    #[test]
    fn serde() {
        let srv = Service::new("test", Command::new("sh", ["-c", "sleep 300"]));
        let data = serde_yaml::to_string(&srv).unwrap();
        assert_eq!(
            serde_yaml::from_str::<Service>(&data).unwrap().command,
            srv.command
        );
    }
}
