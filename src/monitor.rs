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
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use crate::{
    monitor::{scheduler::SchedulerEvent, watcher::WatcherTrait},
    service::{Info, Service, ServiceId, Stats, Status},
    utils::{
        self,
        libc::{getpid, gettid, setsid, waitpid},
        serializers::instant::check_ref_time,
        signal::{SIGALRM, SIGCHLD, SIGHUP, SIGINT, SIGTERM, Signal, SignalSet, Timer},
    },
};

#[cfg(target_os = "linux")]
use crate::utils::libc::set_child_subreaper;

mod sysinfo;
use sysinfo::Sysinfo;

pub mod scheduler;
use scheduler::Scheduler;

pub mod watcher;
use watcher::Watcher;

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Monitor {
    #[serde(with = "humantime_serde")]
    pub stats_interval: std::time::Duration,
    #[serde(with = "humantime_serde")]
    pub restart_interval: std::time::Duration,
    #[serde(with = "humantime_serde")]
    pub clock_check_interval: std::time::Duration,
    #[serde(with = "utils::serializers::service_dashmap")]
    pub services: DashMap<ServiceId, Arc<Service>>,
    #[serde(skip)]
    pub scheduler: Scheduler,
    #[serde(skip)]
    watcher: Mutex<Option<Watcher>>,
    #[serde(skip)]
    sysinfo: Mutex<Sysinfo>,
    #[serde(skip)]
    _stats: Mutex<Arc<Stats>>,
    #[serde(skip)]
    start_time: Instant,
}

impl Default for Monitor {
    fn default() -> Self {
        Self {
            stats_interval: Duration::from_secs(10),
            restart_interval: Duration::from_secs(1),
            clock_check_interval: Duration::from_hours(1),
            services: Default::default(),
            scheduler: Default::default(),
            watcher: Default::default(),
            sysinfo: Default::default(),
            _stats: Default::default(),
            start_time: Instant::now(),
        }
    }
}

impl Monitor {
    #[inline]
    pub fn new() -> Arc<Self> {
        Arc::default()
    }

    #[tracing::instrument()]
    pub fn init() -> Result<()> {
        // block signal before spawning threads to apply mask to all threads
        (SignalSet::default() + SIGALRM + SIGCHLD + SIGTERM + SIGINT).block()?;

        if let Err(err) = setsid() {
            tracing::error!(?err, "setsid failed");
        }
        #[cfg(target_os = "linux")]
        {
            if let Err(err) = set_child_subreaper(getpid()) {
                tracing::error!(?err, "failed to set child-subreaper");
            }
        }
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub fn on_sigchld(&self) {
        self.waitpid(-1);
    }

    fn waitpid(&self, pid: libc::pid_t) {
        while let Some((pid, status)) = waitpid(pid, false) {
            if let Some(service) = self.find_by_pid(pid) {
                if libc::WIFSIGNALED(status) {
                    let signal = Signal(libc::WTERMSIG(status));
                    tracing::info!(
                        id = service.id,
                        name = service.name,
                        pid,
                        ?signal,
                        "service terminated by signal"
                    );

                    if signal == SIGTERM {
                        service.set_finished();
                    } else {
                        service.set_crashed();
                    }
                } else if libc::WIFEXITED(status) {
                    let code = libc::WEXITSTATUS(status);
                    tracing::info!(
                        id = service.id,
                        name = service.name,
                        pid,
                        code,
                        "service exited"
                    );

                    if code == 0 {
                        service.set_finished();
                    } else {
                        service.set_crashed();
                    }
                } else if libc::WIFSTOPPED(status) {
                    service.set_stopped();
                } else if libc::WIFCONTINUED(status) {
                    service.set_running(pid);
                }

                if let Status::Crashed = service.info().status {
                    self.scheduler.schedule_restart(&service, self);
                }
            } else {
                tracing::warn!(pid, "unknown process");
            }
        }
    }

    fn next_restart(&self, info: &Info) -> Instant {
        info.end_time.unwrap_or_else(Instant::now)
            + self.restart_interval * (1 << (info.crashed - 1))
    }

    #[tracing::instrument(skip(self))]
    pub fn process(&self) {
        for event in self.scheduler.iter() {
            match event {
                SchedulerEvent::ServiceSchedule { id, date_time, .. } => {
                    if let Some(service) = self.get(&id)
                        && service.info().active
                    {
                        service.restart();
                        self.scheduler.reschedule(&service, Some(date_time));
                    } else {
                        tracing::warn!(id, "unknown service");
                    }
                }
                SchedulerEvent::ServiceRestart { id, .. } => {
                    if let Some(service) = self.get(&id) {
                        if service.info().active {
                            service.restart();
                        } else {
                            tracing::warn!(id, "not restarting inactive service");
                        }
                    } else {
                        tracing::warn!(id, "unknown service");
                    }
                }
                SchedulerEvent::Sysinfo { instant } => {
                    self.sysinfo.lock().unwrap().update(self);
                    self.scheduler.enqueue(SchedulerEvent::Sysinfo {
                        instant: instant + self.stats_interval,
                    });
                }
                SchedulerEvent::ClockCheck { instant } => {
                    if !check_ref_time() {
                        tracing::info!("refreshing scheduler");
                        self.scheduler.init(self);
                    }
                    self.scheduler.enqueue(SchedulerEvent::ClockCheck {
                        instant: instant + self.clock_check_interval,
                    });
                }
            }
        }
    }

    pub fn run(self: &Arc<Self>) -> Result<()> {
        let sigset = SignalSet::default() + SIGALRM + SIGCHLD + SIGTERM + SIGHUP + SIGINT;
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
            self.add_watch(srv.as_ref())
                .unwrap_or_else(|err| tracing::error!(?err, "watcher failure"));
        }

        self.scheduler.init(self);
        let mut timer = Timer::new(Duration::from_millis(1), false);
        timer.start()?;

        loop {
            let _span = tracing::info_span!(parent: None, "monitor").entered();

            if let Some(duration) = self.scheduler.peek() {
                tracing::debug!(?duration, "sleeping for");
                timer.set_duration(duration.max(Duration::from_millis(1)));
                timer.start()?;
            }

            match sigset.wait()? {
                SIGALRM => {
                    tracing::trace!("processing events");
                    self.process();
                }
                SIGHUP => {
                    tracing::info!("refreshing scheduler");
                    self.scheduler.init(self);
                }
                SIGCHLD => self.on_sigchld(),
                signal @ (SIGTERM | SIGINT) => {
                    tracing::info!("termination requested ({:?})", signal);
                    // timer.stop()?;
                    for service in self.services.iter() {
                        if let Some(pid) = service.info().pid {
                            tracing::trace!(
                                id = service.id,
                                name = service.name,
                                pid,
                                "stopping service"
                            );
                            let _ = Signal::kill(pid, SIGTERM);
                        }
                    }
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

    pub fn get(&self, id: &ServiceId) -> Option<Arc<Service>> {
        self.services.get(id).map(|x| Arc::clone(&x))
    }

    pub fn insert(self: &Arc<Self>, service: Service) -> Arc<Service> {
        let id = service.id;
        let service = Arc::new(service);
        self.services.insert(id, Arc::clone(&service));
        self.scheduler.reschedule(&service, None);
        self.add_watch(&service)
            .unwrap_or_else(|err| tracing::error!(?err, "watcher failure"));
        service
    }

    fn add_watch(self: &Arc<Self>, service: &Service) -> Result<()> {
        if let Some(watch) = service.watch.as_ref() {
            let mut guard = self.watcher.lock().unwrap();
            let watcher = match guard.as_mut() {
                Some(w) => w,
                None => guard.insert(Watcher::new(Arc::downgrade(self))?),
            };
            watcher.add(&service.id, watch)?;
        }
        Ok(())
    }

    fn remove_watch(self: &Arc<Self>, service_id: &ServiceId) {
        let mut guard = self.watcher.lock().unwrap();
        if let Some(watcher) = guard.as_mut() {
            watcher.remove(service_id);
        }
    }

    pub fn remove(self: &Arc<Self>, service_id: &ServiceId) {
        self.services.remove(service_id);
        /* don't wake, worst case there'll be a spurious wakeup */
        self.scheduler.remove(service_id);
        self.remove_watch(service_id);
    }

    pub fn stats(&self) -> Arc<Stats> {
        Arc::clone(&self._stats.lock().unwrap())
    }
}

extern "C" fn blocked_sighandler(sig: libc::c_int) {
    tracing::error!(
        sig = ?Signal(sig),
        pid = getpid(),
        tid = gettid(),
        "blocked signal caught"
    );
    panic!("blocked signal caught: {}", sig);
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{
        service::{Command, Status},
        utils::signal::{SIGALRM, SIGCHLD, SIGKILL, SIGTERM, Signal, SignalSet},
    };
    use anyhow::Result;
    use serial_test::serial;

    #[ctor::ctor]
    fn prepare() {
        use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
        tracing_subscriber::Registry::default()
            .with(tracing_subscriber::EnvFilter::from_default_env())
            // .with(tracing_subscriber::fmt::layer().with_thread_ids(true)) // thread debugging
            .with(tracing_forest::ForestLayer::default())
            .try_init();

        // rust test framewrok uses threads, the main process may handle signals
        (SignalSet::empty() + SIGALRM + SIGTERM + SIGCHLD + SIGINT).block();
    }

    #[test]
    #[serial(waitpid)]
    fn check() {
        let mon = Arc::<Monitor>::default();
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
        let sigset = SignalSet::default() + SIGALRM + SIGCHLD + SIGTERM;

        for sig in &sigset {
            sig.set_handler(blocked_sighandler as usize)?;
        }

        sigset.block()?;
        let srv = Service::new("test_crash", Command::new("false", ["-la"]));

        srv.start();

        assert_eq!(sigset.wait()?, SIGCHLD);
        Monitor::default().on_sigchld();

        sigset.restore()
    }

    #[test]
    #[serial(waitpid)]
    /// ensure that signals are unblocked for the child process
    fn sigterm_child() -> Result<()> {
        let mon = Arc::<Monitor>::default();
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
            Some(pid) => Signal::kill(pid, SIGKILL)?,
            None => panic!("process not started"),
        };
        std::thread::sleep(mon.restart_interval * 2);
        assert_ne!(1, service.info().restarts);

        Signal::kill(getpid(), SIGTERM)?;
        join_handle.join().unwrap()?;
        Ok(())
    }

    #[test]
    #[serial(waitpid)]
    fn run() -> Result<()> {
        let mon = Arc::new(Monitor {
            stats_interval: std::time::Duration::from_millis(100),
            ..Default::default()
        });
        mon.insert(Service::new("test_crash", Command::new("false", ["-la"])));

        let join_handle = {
            let mon = Arc::clone(&mon);
            std::thread::spawn(move || mon.run())
        };
        std::thread::sleep(std::time::Duration::from_millis(300));
        Signal::kill(getpid(), SIGTERM)?;

        join_handle.join().unwrap()?;

        assert!(mon.services.iter().next().unwrap().info().restarts >= 1);
        Ok(())
    }

    #[test]
    #[serial(waitpid)]
    fn stopped() -> Result<()> {
        let mon = Arc::new(Monitor {
            stats_interval: std::time::Duration::from_millis(250),
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

        Signal::kill(info.pid.unwrap(), Signal(libc::SIGSTOP))?;
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert_eq!(service.info().status, Status::Stopped);

        Signal::kill(info.pid.unwrap(), Signal(libc::SIGCONT))?;
        std::thread::sleep(mon.stats_interval * 2);

        assert_eq!(service.info().status, Status::Running);

        Signal::kill(getpid(), SIGTERM)?;

        join_handle.join().unwrap()?;
        Ok(())
    }
}
