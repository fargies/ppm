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

use crate::utils::{
    self,
    signal::{self, Signal, SignalSet},
};
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::process;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use std::{os::unix::process::CommandExt, time::Duration};

mod command;
pub use command::Command;

mod info;
pub use info::Info;

mod stats;
pub use stats::Stats;

mod status;
pub use status::Status;

static S_ID: AtomicUsize = AtomicUsize::new(0);
pub const SERVICE_ID_INVALID: usize = usize::MAX;

pub type ServiceId = usize;

mod tabled;

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Service {
    #[serde(skip_serializing_if = "is_invalid_id")]
    pub id: ServiceId,
    pub name: String,
    pub command: Command,
    #[serde(skip)]
    _info: Mutex<Arc<Info>>,
    #[serde(skip)]
    _stats: Mutex<Arc<Stats>>,
}

fn is_invalid_id(id: &ServiceId) -> bool {
    id == &SERVICE_ID_INVALID
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
            _info: Default::default(),
            _stats: Default::default(),
        }
    }

    pub fn validate(mut self) -> Result<Self> {
        if self.id == SERVICE_ID_INVALID {
            self.id = S_ID.fetch_add(1, Ordering::Relaxed);
        } else {
            S_ID.fetch_max(self.id + 1, Ordering::Relaxed);
        }
        if self.command.path.is_empty() {
            return Err(anyhow!("invalid command, missing `path`"));
        } else if self.name.is_empty() {
            return Err(anyhow!("service `name` missing"));
        }
        Ok(self)
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
                SignalSet::full()
                    .restore()
                    .expect("failed to restore default signal handlers");
                let mut cmd = process::Command::new(self.command.path.as_str());
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
                std::process::exit(-1);
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
        {
            let mut guard = self._info.lock().unwrap();
            Arc::make_mut(&mut guard).active = false;
        }

        if let Some(pid) = self.info().pid {
            tracing::debug!(pid, "trying to stop");
            if utils::waitpid(pid).is_some() {
                tracing::info!(pid = pid, "process (already) terminated");
            } else if self.terminate(signal::SIGTERM, &Duration::from_secs(5)) {
                tracing::info!(pid = pid, "process terminated");
            } else if self.terminate(signal::SIGKILL, &Duration::from_secs(10)) {
                tracing::info!(pid = pid, "process killed");
            } else {
                tracing::error!("failed to kill process");
            }
        }
    }

    /// send a termination signal, wait for process end
    #[tracing::instrument(fields(name=self.name, id=self.id), skip(self))]
    fn terminate(&self, signal: Signal, timeout: &Duration) -> bool {
        if let Some(pid) = self.info().pid {
            let _ = Signal::kill(pid, signal);

            let start = std::time::Instant::now();
            loop {
                if let Some((_, status)) = utils::waitpid(pid) {
                    self.on_waitpid(pid, status);
                } else if self.info().pid.is_none() {
                    return true;
                } else if &start.elapsed() < timeout {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                } else {
                    return false;
                }
            }
        }
        true
    }

    #[tracing::instrument(fields(name=self.name, id=self.id), skip(self))]
    pub fn on_waitpid(&self, pid: libc::pid_t, status: libc::c_int) {
        if libc::WIFSIGNALED(status) {
            let signal = signal::Signal(libc::WTERMSIG(status));
            tracing::debug!(?signal, pid, "process killed");

            if signal == signal::SIGTERM {
                self.set_finished();
            } else {
                self.set_crashed();
            }
        } else if libc::WIFEXITED(status) {
            let code = libc::WEXITSTATUS(status);
            tracing::debug!(code, pid, "process exited");
            if code == 0 {
                self.set_finished();
            } else {
                self.set_crashed();
            }
        } else if libc::WIFSTOPPED(status) {
            tracing::debug!(pid, "process stopped");
            self.set_stopped();
        } else if libc::WIFCONTINUED(status) {
            tracing::debug!(pid, "process continued");
            self.set_running(pid);
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

    pub fn update_stats(&self, stats: Stats) {
        *self._stats.lock().unwrap() = Arc::new(stats);
    }
}

impl Default for Service {
    fn default() -> Self {
        Self {
            id: SERVICE_ID_INVALID,
            name: Default::default(),
            command: Default::default(),
            _info: Default::default(),
            _stats: Default::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        monitor::Monitor,
        utils::signal::{SIGALRM, SIGCHLD, SIGTERM},
    };

    use super::*;
    use serial_test::serial;

    #[ctor::ctor]
    fn prepare() {
        // rust test framewrok uses threads, the main process may handle signals
        (SignalSet::empty() + SIGALRM + SIGTERM + SIGCHLD).block();
    }

    #[test]
    #[serial(waitpid)]
    fn spawn() -> Result<()> {
        (SignalSet::empty() + SIGCHLD).block()?;
        let service = Service::new("test", Command::new("ls", ["-la"]));
        service.start();
        let mon = Monitor::default();
        let service = mon.insert(service);

        assert!(service.info().pid.is_some_and(|pid| pid > 0));
        assert_eq!(service.info().status, Status::Running);

        // wait for command to terminate
        std::thread::sleep(Duration::from_millis(100));

        mon.on_sigchld();

        assert_eq!(service.info().pid, None);
        assert_eq!(service.info().status, Status::Finished);

        service.stop();
        assert!(!service.info().active);
        Ok(())
    }

    #[test]
    #[serial(waitpid)]
    fn stop() -> Result<()> {
        (SignalSet::empty() + SIGCHLD).block()?;
        let service = Service::new("test", Command::new("sh", ["-c", "sleep 300"]));
        service.start();
        let mon = Monitor::default();
        let service = mon.insert(service);

        assert!(service.info().pid.is_some_and(|pid| pid > 0));
        assert_eq!(service.info().status, Status::Running);

        service.stop();
        assert!(!service.info().active);
        // wait for command to terminate
        std::thread::sleep(Duration::from_millis(100));

        mon.on_sigchld();
        assert_eq!(service.info().pid, None);
        assert_eq!(service.info().status, Status::Finished);
        Ok(())
    }

    #[test]
    fn serde() {
        let srv = Service::new("test", Command::new("sh", ["-c", "sleep 300"]));
        let data = serde_yaml_ng::to_string(&srv).unwrap();
        assert_eq!(
            serde_yaml_ng::from_str::<Service>(&data).unwrap().command,
            srv.command
        );
    }
}
