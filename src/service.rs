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
use libc::{WEXITSTATUS, WIFCONTINUED, WIFEXITED, WIFSIGNALED, WIFSTOPPED, WTERMSIG, c_int, pid_t};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use std::{
    env::{current_dir, current_exe},
    ops::Deref,
    path::PathBuf,
    process,
    sync::{
        Arc, LazyLock, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use crate::monitor::logger::Logger;
use crate::utils::libc::waitpid;
use crate::utils::signal::{self, SIGTERM, Signal};

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

static LAUNCHER_EXE: LazyLock<Option<PathBuf>> = LazyLock::new(|| {
    match current_exe()
        .ok()
        .and_then(|mut path| {
            path.set_file_name("ppm-launcher");
            if path.exists() { Some(path) } else { None }
        })
        .or_else(|| {
            current_dir()
                .ok()
                .map(|path| path.join("target/debug/ppm-launcher"))
                .filter(|p| p.exists())
        })
        .or_else(|| {
            current_dir()
                .ok()
                .map(|path| path.join("target/release/ppm-launcher"))
                .filter(|p| p.exists())
        }) {
        ret @ Some(_) => ret,
        None => {
            tracing::error!("no launcher found");
            #[cfg(test)]
            tracing::error!(
                "invoke `cargo build` to compile the ppm-launcher before running tests"
            );
            None
        }
    }
});

fn get_service_id_default() -> usize {
    SERVICE_ID_INVALID
}

#[derive(Serialize, Deserialize)]
pub struct Service {
    /// Service ID
    #[serde(
        skip_serializing_if = "is_invalid_id",
        default = "get_service_id_default"
    )]
    pub id: ServiceId,
    /// Service name
    #[serde(default)]
    pub name: String,
    /// Command to run
    #[serde(default)]
    pub command: Command,
    /// Workdir for the service
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub workdir: Option<String>,
    /// Command schedule for periodic commands
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub schedule: Option<Cron>,
    /// Directory watchs to monitor
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub watch: Option<Watch>,
    /// Running process informations
    #[serde(skip, default)]
    _info: Mutex<Arc<Info>>,
    /// Running process statistics
    #[serde(skip, default)]
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

    #[tracing::instrument(level = "TRACE", fields(name=self.name, id=self.id), skip(self, logger))]
    pub fn start<'a, L>(&self, logger: L)
    where
        L: Into<Option<&'a Logger>>,
    {
        self.restart(logger)
    }

    #[tracing::instrument(level = "INFO", fields(name=self.name, id=self.id), skip(self, logger), ret(level = "TRACE"))]
    pub fn restart<'a, L>(&self, logger: L)
    where
        L: Into<Option<&'a Logger>>,
    {
        if self.info().pid.is_some() {
            self.stop();
        }

        let launcher = match LAUNCHER_EXE.deref().as_ref() {
            Some(launcher) => launcher,
            None => {
                tracing::error!("no launcher available");
                return;
            }
        };

        // Lock the service info, may block clients for the time a service is
        // restarted but will prevent monitor from running waitpid
        // before we've set pid on this service
        let mut guard = self._info.lock().unwrap();
        let (out, err) = logger
            .into()
            .and_then(|l| l.make_pipe(self).ok())
            .unwrap_or_else(|| (process::Stdio::inherit(), process::Stdio::inherit()));

        let mut cmd = process::Command::new(launcher);
        cmd.arg(self.command.path.as_str())
            .args(&self.command.args)
            .stdin(process::Stdio::null())
            .stdout(out)
            .stderr(err);
        if let Some(workdir) = self.workdir.as_ref() {
            cmd.current_dir(workdir);
        }
        if let Some(env) = self.command.env.as_ref() {
            cmd.envs(env);
        }

        match cmd.spawn() {
            Ok(child) => {
                let info = Arc::make_mut(&mut guard);
                info.active = true;
                info.set_running(child.id() as pid_t);
            }
            Err(err) => tracing::error!(?err, "failed to spawn process"),
        }
    }

    #[tracing::instrument(level = "INFO", fields(name=self.name, id=self.id), skip(self), ret(level = "TRACE"))]
    pub fn set_active(&self, value: bool) {
        let mut guard = self._info.lock().unwrap();
        Arc::make_mut(&mut guard).active = value;
    }

    /// Stop the process
    ///
    /// May timeout when called from [Monitor] thread, waiting for zombie to be
    /// released.
    #[tracing::instrument(level = "INFO", fields(name=self.name, id=self.id), skip(self), ret(level = "TRACE"))]
    pub fn stop(&self) {
        {
            let mut guard = self._info.lock().unwrap();
            Arc::make_mut(&mut guard).active = false;
        }

        if let Some(pid) = self.info().pid {
            tracing::debug!(pid, "trying to stop");
            if self.terminate(pid, SIGTERM, &Duration::from_secs(5)) {
                tracing::trace!(pid, "process terminated");
            } else if self.terminate(pid, signal::SIGKILL, &Duration::from_secs(10)) {
                tracing::trace!(pid, "process killed");
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
    #[tracing::instrument(level = "INFO", fields(name=self.name, id=self.id), skip(self), ret)]
    fn terminate(&self, pid: pid_t, signal: Signal, timeout: &Duration) -> bool {
        if Signal::kill(pid, signal).is_err() {
            // already dead
            return true;
        }

        let start = std::time::Instant::now();
        while self.info().pid.is_some_and(|p| pid == p) {
            if let Some((pid, status)) = waitpid(pid, false) {
                self.set_terminated(pid, status);
                return true;
            } else if &start.elapsed() < timeout {
                std::thread::sleep(std::time::Duration::from_millis(10));
            } else {
                return false;
            }
        }
        true
    }

    #[tracing::instrument(level = "INFO", fields(name=self.name, id=self.id), skip(self), ret(level = "TRACE"))]
    pub fn set_terminated(&self, pid: pid_t, status: c_int) -> Status {
        let mut guard = self._info.lock().unwrap();
        if guard.pid.is_none_or(|p| p != pid) {
            tracing::trace!(
                id = self.id,
                name = self.name,
                terminated_pid = pid,
                pid = guard.pid,
                "invalid pid for service"
            );
        } else if WIFSIGNALED(status) {
            let signal = Signal(WTERMSIG(status));
            tracing::info!(
                id = self.id,
                name = self.name,
                pid,
                ?signal,
                "service terminated by signal"
            );

            if signal == SIGTERM {
                Arc::make_mut(&mut guard).set_finished();
            } else {
                Arc::make_mut(&mut guard).set_crashed();
            }
        } else if WIFEXITED(status) {
            let code = WEXITSTATUS(status);
            tracing::info!(id = self.id, name = self.name, pid, code, "service exited");

            if code == 0 {
                Arc::make_mut(&mut guard).set_finished();
            } else {
                Arc::make_mut(&mut guard).set_crashed();
            }
        } else if WIFSTOPPED(status) {
            Arc::make_mut(&mut guard).set_stopped();
        } else if WIFCONTINUED(status) {
            Arc::make_mut(&mut guard).set_running(pid);
        }

        guard.status
    }

    /// Set service as [Status::Crashed]
    ///
    /// Must be called from [Monitor]
    #[tracing::instrument(level = "INFO", fields(name=self.name, id=self.id), skip(self))]
    pub fn set_crashed(&self) {
        let mut guard = self._info.lock().unwrap();
        Arc::make_mut(&mut guard).set_crashed();
    }

    /// Set service as [Status::Finished]
    ///
    /// Must be called from [Monitor]
    #[tracing::instrument(level = "INFO", fields(name=self.name, id=self.id), skip(self))]
    pub fn set_finished(&self) {
        let mut guard = self._info.lock().unwrap();
        Arc::make_mut(&mut guard).set_finished();
    }

    /// Set service as [Status::Stopped]
    ///
    /// Must be called from [Monitor]
    #[tracing::instrument(level = "INFO", fields(name=self.name, id=self.id), skip(self))]
    pub fn set_stopped(&self) {
        let mut guard = self._info.lock().unwrap();
        Arc::make_mut(&mut guard).set_stopped();
    }

    /// Set service as [Status::Running]
    ///
    /// Must be called from [Monitor]
    #[tracing::instrument(level = "INFO", fields(name=self.name, id=self.id), skip(self))]
    pub fn set_running(&self, pid: pid_t) {
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

impl Drop for Service {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "linux")]
    use crate::utils::libc::waitpid;
    use crate::utils::signal::SignalSet;
    #[cfg(target_os = "linux")]
    use std::{
        fs::File,
        io::{Read, Seek, Write},
    };

    use crate::{
        monitor::Monitor,
        utils::{
            kill_on_drop,
            signal::{SIGALRM, SIGCHLD, SIGTERM},
            wait_for,
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
        service.start(None);
        let mon = Arc::new(Monitor::default());
        let service = mon.insert(service);

        assert!(service.info().pid.is_some_and(|pid| pid > 0));
        assert_eq!(service.info().status, Status::Running);

        wait_for!(
            mon.on_sigchld() != 0 && service.info().pid.is_none(),
            "pid:{:?}",
            service.info().pid
        )
        .expect("service should terminate");

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
        service.start(None);
        let mon = Arc::new(Monitor::default());
        let service = mon.insert(service);

        let join_handle = {
            /* Monitor is handling dead processes */
            let mon = Arc::clone(&mon);
            std::thread::spawn(move || mon.run())
        };
        let _drop_guard = kill_on_drop(join_handle);

        assert!(service.info().pid.is_some_and(|pid| pid > 0));
        assert_eq!(service.info().status, Status::Running);

        service.stop();
        assert!(!service.info().active);

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

    #[test]
    #[serial(waitpid)]
    #[cfg(target_os = "linux")]
    fn workdir() -> Result<()> {
        (SignalSet::empty() + SIGCHLD).block()?;
        let temp_dir = std::env::temp_dir();
        let mut srv = Service::new("test", Command::new("sh", ["-c", "sleep 300"]));
        srv.workdir = temp_dir.to_str().map(str::to_owned);
        srv.start(None);
        let mon = Arc::new(Monitor::default());
        let service = mon.insert(srv);

        let join_handle = {
            /* Monitor is handling dead processes */
            let mon = Arc::clone(&mon);
            std::thread::spawn(move || mon.run())
        };
        let _drop_guard = kill_on_drop(join_handle);

        wait_for!(service.info().pid.is_some()).expect("not started");
        let pid = service.info().pid.unwrap();
        wait_for!(std::fs::read_link(format!("/proc/{pid}/cwd"))? == temp_dir)
            .expect("failed to switch to workdir");

        service.stop();

        Ok(())
    }

    #[test]
    #[serial(waitpid)]
    #[cfg(target_os = "linux")]
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
                service.restart(None);
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
