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

use anyhow::{Result, anyhow};
use croner::Cron;
use serde::{Deserialize, Serialize};
use std::process;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use std::{os::unix::process::CommandExt, time::Duration};

use crate::utils::signal::{self, SIGTERM, Signal, SignalSet};

mod command;
pub use command::Command;

mod info;
pub use info::Info;

mod stats;
pub use stats::Stats;

mod status;
pub use status::Status;

mod tabled;

mod watch;
pub use watch::Watch;

static S_ID: AtomicUsize = AtomicUsize::new(0);
pub const SERVICE_ID_INVALID: usize = usize::MAX;

pub type ServiceId = usize;

#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct Service {
    /// Service ID
    #[serde(skip_serializing_if = "is_invalid_id")]
    pub id: ServiceId,
    /// Service name
    pub name: String,
    /// Command to run
    pub command: Command,
    /// Workdir for the service
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workdir: Option<String>,
    /// Command schedule for periodic commands
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schedule: Option<Cron>,
    /// Directory watchs to monitor
    #[serde(skip_serializing_if = "Option::is_none")]
    pub watch: Option<Watch>,
    /// Running process informations
    #[serde(skip)]
    _info: Mutex<Arc<Info>>,
    /// Running process statistics
    #[serde(skip)]
    _stats: Mutex<Arc<Stats>>,
}

impl std::fmt::Debug for Service {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut binding = f.debug_struct("Service");
        binding
            .field("id", &self.id)
            .field("name", &self.name)
            .field("command", &self.command);
        if let Some(schedule) = &self.schedule {
            binding.field("schedule", schedule);
        }
        binding.finish()
    }
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
            workdir: None,
            schedule: Default::default(),
            watch: None,
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
        // Lock the service info, may block clients for the time a service is
        // restarted but will prevent monitor from running waitpid
        // before we've set pid on this service
        let mut guard = self._info.lock().unwrap();

        match unsafe { libc::fork() } {
            x if x < 0 => {
                tracing::error!("failed to fork");
            }
            0 => {
                SignalSet::full()
                    .restore()
                    .expect("failed to restore default signal handlers");
                #[cfg(target_os = "linux")]
                if let Err(err) = Signal::set_pdeath_sig(SIGTERM) {
                    tracing::error!(?err, "failed to set pdeath signal");
                }

                let mut cmd = process::Command::new(self.command.path.as_str());
                cmd.args(&self.command.args)
                    .stdin(process::Stdio::null())
                    .stdout(process::Stdio::inherit())
                    .stderr(process::Stdio::inherit());
                if let Some(workdir) = self.workdir.as_ref() {
                    cmd.current_dir(workdir);
                }

                match self.command.env.as_ref() {
                    Some(env) => cmd.envs(env),
                    None => cmd.env_clear(),
                };
                let err = cmd.exec();
                tracing::error!("failed to spawn process: {}", err);
                std::process::exit(-1);
            }
            pid => {
                let info = Arc::make_mut(&mut guard);
                info.active = true;
                info.set_running(pid);
            }
        }
    }

    pub fn set_active(&self, value: bool) {
        let mut guard = self._info.lock().unwrap();
        Arc::make_mut(&mut guard).active = value;
    }

    /// Stop the process
    ///
    /// May timeout when called from [Monitor] thread, waiting for zombie to be
    /// released.
    #[tracing::instrument(fields(name=self.name, id=self.id), skip(self))]
    pub fn stop(&self) {
        {
            let mut guard = self._info.lock().unwrap();
            Arc::make_mut(&mut guard).active = false;
        }

        if let Some(pid) = self.info().pid {
            tracing::debug!(pid, "trying to stop");
            if self.terminate(SIGTERM, &Duration::from_secs(5)) {
                tracing::info!(pid = pid, "process terminated");
            } else if self.terminate(signal::SIGKILL, &Duration::from_secs(10)) {
                tracing::info!(pid = pid, "process killed");
            } else {
                tracing::error!("failed to kill process");
            }
        } else {
            tracing::info!("process (already) terminated");
        }
    }

    /// send a termination signal, wait for process end
    ///
    /// This will not update the service `info`, the `Monitor` thread should
    /// do using `waitpid`
    #[tracing::instrument(fields(name=self.name, id=self.id), skip(self))]
    fn terminate(&self, signal: Signal, timeout: &Duration) -> bool {
        if let Some(pid) = self.info().pid {
            if Signal::kill(pid, signal).is_err() {
                // already dead
                return true;
            }

            let start = std::time::Instant::now();
            loop {
                if !Signal::exists(pid) || self.info().pid.is_none() {
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

    /// Set service as [Status::Crashed]
    ///
    /// Must be called from [Monitor]
    #[tracing::instrument(fields(name=self.name, id=self.id), skip(self))]
    pub fn set_crashed(&self) {
        let mut guard = self._info.lock().unwrap();
        Arc::make_mut(&mut guard).set_crashed();
    }

    /// Set service as [Status::Finished]
    ///
    /// Must be called from [Monitor]
    #[tracing::instrument(fields(name=self.name, id=self.id), skip(self))]
    pub fn set_finished(&self) {
        let mut guard = self._info.lock().unwrap();
        Arc::make_mut(&mut guard).set_finished();
    }

    /// Set service as [Status::Stopped]
    ///
    /// Must be called from [Monitor]
    #[tracing::instrument(fields(name=self.name, id=self.id), skip(self))]
    pub fn set_stopped(&self) {
        let mut guard = self._info.lock().unwrap();
        Arc::make_mut(&mut guard).set_stopped();
    }

    /// Set service as [Status::Running]
    ///
    /// Must be called from [Monitor]
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
            workdir: None,
            schedule: Default::default(),
            watch: None,
            _info: Default::default(),
            _stats: Default::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs::File,
        io::{Read, Seek, Write},
    };

    use crate::{
        monitor::Monitor,
        utils::{
            libc::{getpid, waitpid},
            signal::{SIGALRM, SIGCHLD, SIGTERM},
        },
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
        let mon = Arc::new(Monitor::default());
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
        let mon = Arc::new(Monitor::default());
        let service = mon.insert(service);

        let join_handle = {
            /* Monitor is handling dead processes */
            let mon = Arc::clone(&mon);
            std::thread::spawn(move || mon.run())
        };

        assert!(service.info().pid.is_some_and(|pid| pid > 0));
        assert_eq!(service.info().status, Status::Running);

        service.stop();
        assert!(!service.info().active);

        assert_eq!(service.info().pid, None);
        assert_eq!(service.info().status, Status::Finished);

        Signal::kill(getpid(), SIGTERM)?;
        join_handle.join().unwrap()?;
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

    #[test]
    #[serial(waitpid)]
    #[cfg(target_os = "linux")]
    fn workdir() -> Result<()> {
        (SignalSet::empty() + SIGCHLD).block()?;
        let temp_dir = std::env::temp_dir();
        let mut srv = Service::new("test", Command::new("sh", ["-c", "sleep 300"]));
        srv.workdir = temp_dir.to_str().map(str::to_owned);
        srv.start();
        let mon = Arc::new(Monitor::default());
        let service = mon.insert(srv);

        let join_handle = {
            /* Monitor is handling dead processes */
            let mon = Arc::clone(&mon);
            std::thread::spawn(move || mon.run())
        };
        // Wait a bit for fork to exec
        std::thread::sleep(Duration::from_millis(100));
        let pid = service.info().pid.expect("pid should be set");
        assert_eq!(std::fs::read_link(format!("/proc/{pid}/cwd"))?, temp_dir);

        service.stop();

        Signal::kill(getpid(), SIGTERM)?;
        join_handle.join().unwrap()?;
        Ok(())
    }

    #[test]
    #[serial(waitpid)]
    /// Run this test with `-- --no-capture` to see messages from child
    fn child_kill_subreap() -> Result<()> {
        let mut fd = File::options()
            .truncate(true)
            .read(true)
            .write(true)
            .create(true)
            .open(std::env::temp_dir().join("ppm_test_child_death.txt"))?;

        Monitor::init()?;

        match unsafe { libc::fork() } {
            x if x < 0 => panic!("failed to fork"),
            0 => {
                let service = Service::new(
                    "test",
                    Command::new("sh", ["-c", "trap 'child died' EXIT; sleep 300"]),
                );
                /* fork/exec */
                service.restart();
                write!(fd, "{}", service.info().pid.expect("should have spawned"))
                    .expect("failed to write");
                drop(fd);
                /* wait for process to actually setup death-sig before dying */
                std::thread::sleep(Duration::from_millis(150));
                std::process::exit(0);
            }
            pid => {
                let (ret, _) = waitpid(pid, true).expect("should have resolved");
                assert_eq!(pid, ret);

                fd.rewind()?;
                let child_pid = {
                    let mut buf = String::new();
                    fd.read_to_string(&mut buf)?;
                    drop(fd);
                    buf.parse::<libc::pid_t>()?
                };
                assert_ne!(0, child_pid);
                std::thread::sleep(Duration::from_millis(100));
                while let Some((pid, _)) = waitpid(-1, false) {
                    /* this should happen if current process has correctly been set as a subreaper */
                    tracing::info!(pid, "collected");
                }
                /* process should have been collected, and it should have received SIGKILL (on Linux only) */
                #[cfg(target_os = "linux")]
                assert_eq!(-1, unsafe { libc::kill(child_pid, 0) });
            }
        }
        Ok(())
    }
}
