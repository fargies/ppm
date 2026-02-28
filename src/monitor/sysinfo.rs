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
** Created on: 2025-12-31T12:10:20
** Author: Sylvain Fargier <fargier.sylvain@gmail.com>
*/

use dashmap::DashMap;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use sysinfo::{Pid, Process, ProcessRefreshKind, ProcessesToUpdate, System};

use crate::{
    service::{Service, ServiceId, Stats},
    utils::libc::getpid,
};

use super::Monitor;

pub struct Sysinfo {
    system: System,
    pids: Vec<Pid>,
    pub last_update: Instant,
}

impl std::fmt::Debug for Sysinfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sysinfo")
            .field("last_update", &self.last_update)
            .finish()
    }
}

impl Default for Sysinfo {
    fn default() -> Self {
        Self {
            system: System::new(),
            last_update: Instant::now(),
            pids: Vec::with_capacity(10),
        }
    }
}

impl Sysinfo {
    #[tracing::instrument(skip(self, monitor))]
    pub fn update(&mut self, monitor: &Monitor) {
        tracing::info!("updating stats");
        self.fetch(&monitor.services);
        self.update_services(&monitor.services);

        if let Some(proc) = self.system.process(Pid::from(getpid() as usize)) {
            let mut stats = monitor._stats.lock().unwrap();
            *stats = Arc::new(self.make_stats(proc, &stats, Some(monitor.start_time.elapsed())));
        } else {
            tracing::warn!("failed to update daemon stats");
        }
    }

    fn make_stats(&self, proc: &Process, old: &Stats, uptime: Option<Duration>) -> Stats {
        let disk_usage = proc.disk_usage();
        let mut stats = Stats {
            cpu_usage: proc.cpu_usage(),
            cpu_time: Duration::from_millis(proc.accumulated_cpu_time()),
            mem_rss: proc.memory(),
            mem_vsz: proc.virtual_memory(),
            total_io_read: disk_usage.total_read_bytes,
            total_io_write: disk_usage.total_written_bytes,
            uptime,
            ..Default::default()
        };

        if let Some(interval) = stats.uptime.and_then(|new_uptime| {
            old.uptime
                .map(|old_uptime| new_uptime.saturating_sub(old_uptime))
        }) && !interval.is_zero()
        {
            let interval = interval.as_secs_f64();
            stats.io_read = ((disk_usage.total_read_bytes - old.total_io_read) as f64 / interval)
                .round() as u64;
            stats.io_write = ((disk_usage.total_written_bytes - old.total_io_write) as f64
                / interval)
                .round() as u64;
        }

        stats
    }

    #[tracing::instrument(skip(self, services))]
    fn update_services(&self, services: &DashMap<ServiceId, Arc<Service>>) {
        for srv in services {
            let info = srv.info();

            if let Some(proc) = info
                .pid
                .and_then(|p| self.system.process(Pid::from(p as usize)))
            {
                tracing::trace!(id = srv.id, name = srv.name, "updating");
                match proc.status() {
                    sysinfo::ProcessStatus::Idle => srv.set_running(info.pid.unwrap()),
                    sysinfo::ProcessStatus::Run => srv.set_running(info.pid.unwrap()),
                    sysinfo::ProcessStatus::Sleep => srv.set_running(info.pid.unwrap()),
                    sysinfo::ProcessStatus::Stop => srv.set_stopped(),
                    sysinfo::ProcessStatus::Zombie => (/* [Monitor] will handle this */),
                    sysinfo::ProcessStatus::Dead => (/* [Monitor] will handle this */),
                    _ => {}
                }

                srv.update_stats(self.make_stats(
                    proc,
                    &srv.stats(),
                    info.start_time.map(|t| t.elapsed()),
                ));
            } else {
                let stats = srv.stats();
                if stats.uptime.is_some() {
                    srv.update_stats(Stats::default());
                }

                if info.pid.is_some() {
                    tracing::warn!(pid = info.pid, "process looks dead");
                }
            }
        }
    }

    #[tracing::instrument(skip(self, services))]
    fn fetch(&mut self, services: &DashMap<ServiceId, Arc<Service>>) {
        let processes = self.system.processes();

        for pid in services
            .iter()
            .filter_map(|srv| srv.info().pid.map(|p| Pid::from(p as usize)))
            .filter(|p| !processes.contains_key(p))
        {
            self.pids.push(pid);
        }

        let daemon_pid = Pid::from(getpid() as usize);
        if !processes.contains_key(&daemon_pid) {
            self.pids.push(daemon_pid);
        }

        tracing::trace!(pids = ?self.pids, "fetching info");
        self.system.refresh_processes_specifics(
            ProcessesToUpdate::Some(self.pids.as_slice()),
            true,
            ProcessRefreshKind::nothing()
                .with_cpu()
                .with_memory()
                .with_disk_usage(),
        );

        self.pids.clear();
        let processes = self.system.processes();
        for pid in processes.keys() {
            self.pids.push(*pid);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        service::{Command, Status},
        utils::kill_on_drop,
    };
    use anyhow::Result;
    use serial_test::serial;

    use super::*;

    #[test]
    #[serial(waitpid)] // raises SIGCHLD
    fn update() -> Result<()> {
        let mon = Arc::new(Monitor::default());
        {
            let srv = Service::new("test", Command::new("sleep", ["300"]));
            srv.start(None);
            mon.services.insert(0, srv.into());
        }

        let join_handle = {
            /* Monitor is handling dead processes */
            let mon = Arc::clone(&mon);
            std::thread::spawn(move || mon.run())
        };
        let _drop_guard = kill_on_drop(join_handle);

        let mut sysinfo = Sysinfo::default();
        sysinfo.update(&mon);
        assert_eq!(2, sysinfo.pids.len());
        assert_eq!(2, sysinfo.system.processes().len());
        for service in &mon.services {
            service.stop(); /* stop should wait until process is handled by monitor */
        }

        sysinfo.update(&mon);
        assert_eq!(1, sysinfo.pids.len());
        assert_eq!(1, sysinfo.system.processes().len());
        for service in &mon.services {
            assert_eq!(Status::Finished, service.info().status);
        }

        Ok(())
    }
}
