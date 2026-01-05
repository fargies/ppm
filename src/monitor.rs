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
** Created on: 2025-12-23T09:13:19
** Author: Sylvain Fargier <fargier.sylvain@gmail.com>
*/

use anyhow::Result;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

use crate::{
    service::{Info, Service, ServiceId, Status},
    utils::{
        self,
        signal::{self, SignalSet, Timer},
    },
};

mod sysinfo;
use sysinfo::Sysinfo;

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Monitor {
    #[serde(with = "humantime_serde")]
    pub interval: std::time::Duration,
    #[serde(with = "utils::serializers::service_dashmap")]
    pub services: DashMap<ServiceId, Arc<Service>>,
    #[serde(skip)]
    sysinfo: Mutex<Sysinfo>,
}

impl Default for Monitor {
    fn default() -> Self {
        Self {
            interval: std::time::Duration::from_secs(1),
            services: Default::default(),
            sysinfo: Default::default(),
        }
    }
}

impl Monitor {
    #[tracing::instrument(skip(self))]
    fn on_sigchld(&self) {
        while let Some((pid, status)) = utils::waitpid(-1) {
            if libc::WIFSIGNALED(status) {
                let signal = signal::Signal(libc::WTERMSIG(status));
                tracing::debug!(?signal, pid, "process killed");
                if let Some(service) = self.find_by_pid(pid) {
                    if signal == signal::SIGTERM {
                        service.set_finished();
                    } else {
                        service.set_crashed();
                    }
                }
            } else if libc::WIFEXITED(status) {
                let code = libc::WEXITSTATUS(status);
                tracing::debug!(code, pid, "process exited");
                if let Some(service) = self.find_by_pid(pid) {
                    if code == 0 {
                        service.set_finished();
                    } else {
                        service.set_crashed();
                    }
                }
            } else if libc::WIFSTOPPED(status) {
                tracing::debug!(pid, "process stopped");
                if let Some(service) = self.find_by_pid(pid) {
                    service.set_stopped();
                }
            } else if libc::WIFCONTINUED(status) {
                tracing::debug!(pid, "process continued");
                if let Some(service) = self.find_by_pid(pid) {
                    service.set_running(pid);
                }
            }
        }
    }

    fn next_restart(&self, info: &Info) -> Option<std::time::SystemTime> {
        info.end_time
            .map(|d| d + self.interval * (1 << (info.restarts - 1)))
    }

    #[tracing::instrument(skip(self))]
    pub fn process(&self) {
        let now = std::time::SystemTime::now();

        for srv in self.services.iter() {
            let info = srv.info();

            tracing::trace!(
                active = info.active,
                status = ?info.status,
                pid = ?info.pid,
                name = srv.name,
                "processing"
            );
            if info.status == Status::Crashed
                && info.active
                && self.next_restart(&info).is_some_and(|next| next <= now)
            {
                srv.restart();
            }
        }
    }

    pub fn run(&self) -> Result<()> {
        let sigset = SignalSet::default() + signal::SIGALRM + signal::SIGCHLD + signal::SIGTERM;
        for sig in &sigset {
            sig.set_handler(blocked_sighandler as usize)?;
        }
        sigset.block()?;
        let _ondrop = utils::OnDrop::new(|| sigset.restore().unwrap());

        for srv in self.services.iter() {
            let info = srv.info();

            if info.status != Status::Running && info.active {
                srv.restart();
            }
        }

        let timer = Timer::new(self.interval, true);
        timer.start()?;
        loop {
            let _span = tracing::info_span!(parent: None, "monitor").entered();

            self.process();

            match sigset.wait()? {
                signal::SIGALRM => {
                    tracing::trace!("timer expired");
                    self.sysinfo.lock().unwrap().update(&self.services);
                }
                signal::SIGCHLD => self.on_sigchld(),
                signal::SIGTERM => {
                    tracing::info!("termination requested (SIGTERM)");
                    return Ok(());
                }
                signal => {
                    tracing::warn!(?signal, "unhandled signal");
                    return Err(anyhow::format_err!("unknown signal: {signal:?}"));
                }
            }
        }
    }

    pub fn find_by_pid(&self, pid: libc::pid_t) -> Option<Arc<Service>> {
        self.services
            .iter()
            .find(|x| x.info().pid.is_some_and(|x| x == pid))
            .map(|x| Arc::clone(&x))
    }

    pub fn find_by_name(&self, name: &String) -> Option<Arc<Service>> {
        self.services
            .iter()
            .find(|x| &x.name == name)
            .map(|x| Arc::clone(&x))
    }

    pub fn get(&self, id: ServiceId) -> Option<Arc<Service>> {
        self.services.get(&id).map(|x| Arc::clone(&x))
    }

    pub fn insert(&self, service: Service) -> Arc<Service> {
        let id = service.id;
        let service = Arc::new(service);
        self.services.insert(id, Arc::clone(&service));
        service
    }
}

extern "C" fn blocked_sighandler(sig: libc::c_int) {
    tracing::error!(
        sig,
        pid = signal::getpid(),
        tid = signal::gettid(),
        "blocked signal caught"
    );
    panic!("blocked signal caught: {}", sig);
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{
        service::{Command, Status},
        utils::signal::{SIGALRM, SIGCHLD, SIGTERM, Signal},
    };
    use anyhow::Result;
    use serial_test::serial;

    #[ctor::ctor]
    fn prepare() {
        // rust test framewrok uses threads, the main process may handle signals
        (SignalSet::empty() + SIGALRM + SIGTERM + SIGCHLD).block();
    }

    #[test]
    #[serial(waitpid)]
    fn check() {
        let mon = Monitor::default();
        mon.insert(Service::new("test_stop", Command::new("ls", ["-la"])));
        mon.insert(Service::new("test_crash", Command::new("false", ["-la"])));
        mon.services.iter().for_each(|s| s.start());
        std::thread::sleep(std::time::Duration::from_millis(300));
        mon.on_sigchld();

        for service in mon.services.iter() {
            let info = service.info();
            tracing::trace!(name = service.name, ?info);
            assert_ne!(info.end_time, None);
            assert_ne!(info.status, Status::Running);
            assert_eq!(info.pid, None);
        }
    }

    #[test]
    #[serial(waitpid)]
    fn sigchld() -> Result<()> {
        let sigset =
            signal::SignalSet::default() + signal::SIGALRM + signal::SIGCHLD + signal::SIGTERM;

        for sig in &sigset {
            sig.set_handler(blocked_sighandler as usize)?;
        }

        sigset.block()?;
        let srv = Service::new("test_crash", Command::new("false", ["-la"]));

        srv.start();

        assert_eq!(sigset.wait()?, signal::SIGCHLD);
        Monitor::default().on_sigchld();

        sigset.restore()
    }

    #[test]
    #[serial(waitpid)]
    /// ensure that signals are unblocked for the child process
    fn sigterm_child() -> Result<()> {
        let mon = Arc::new(Monitor::default());
        let service = mon.insert(Service::new(
            "test_sigterm_child",
            Command::new("sleep", ["300"]),
        ));

        let join_handle = {
            let mon = Arc::clone(&mon);
            std::thread::spawn(move || mon.run())
        };
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert_eq!(1, service.info().restarts);
        match service.info().pid {
            Some(pid) => Signal::kill(pid, signal::SIGKILL)?,
            None => panic!("process not started"),
        };
        std::thread::sleep(mon.interval * 2);
        assert_ne!(1, service.info().restarts);

        Signal::kill(signal::getpid(), signal::SIGTERM)?;
        join_handle.join().unwrap()?;
        Ok(())
    }

    #[test]
    #[serial(waitpid)]
    fn run() -> Result<()> {
        let mon = Arc::new(Monitor {
            interval: std::time::Duration::from_millis(100),
            ..Default::default()
        });
        mon.insert(Service::new("test_crash", Command::new("false", ["-la"])));

        let join_handle = {
            let mon = Arc::clone(&mon);
            std::thread::spawn(move || mon.run())
        };
        std::thread::sleep(std::time::Duration::from_millis(300));
        Signal::kill(signal::getpid(), signal::SIGTERM)?;

        join_handle.join().unwrap()?;

        assert!(mon.services.iter().next().unwrap().info().restarts >= 1);
        Ok(())
    }

    #[test]
    #[serial(waitpid)]
    fn stopped() -> Result<()> {
        let mon = Arc::new(Monitor {
            interval: std::time::Duration::from_millis(250),
            ..Default::default()
        });
        let service = mon.insert(Service::new("test_stopped", Command::new("sleep", ["300"])));

        let join_handle = {
            let mon = Arc::clone(&mon);
            std::thread::spawn(move || mon.run())
        };

        std::thread::sleep(std::time::Duration::from_millis(100));
        let info = service.info();
        assert_ne!(None, info.pid);

        Signal::kill(info.pid.unwrap(), signal::Signal(libc::SIGSTOP))?;
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert_eq!(service.info().status, Status::Stopped);

        Signal::kill(info.pid.unwrap(), signal::Signal(libc::SIGCONT))?;
        std::thread::sleep(mon.interval * 2);

        assert_eq!(service.info().status, Status::Running);

        Signal::kill(signal::getpid(), signal::SIGTERM)?;

        join_handle.join().unwrap()?;
        Ok(())
    }
}
