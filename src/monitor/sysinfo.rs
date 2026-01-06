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

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use dashmap::DashMap;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

use crate::service::{Service, ServiceId, Stats};

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
    #[tracing::instrument(skip(self, services))]
    pub fn update(&mut self, services: &DashMap<ServiceId, Arc<Service>>) {
        self.fetch(services);
        self.update_services(services);
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
                    sysinfo::ProcessStatus::Zombie => srv.set_crashed(),
                    sysinfo::ProcessStatus::Dead => srv.set_crashed(),
                    _ => {}
                }

                let mut stats = Arc::unwrap_or_clone(srv.stats());
                let uptime = info.start_time.and_then(|t| t.elapsed().ok());
                stats.cpu_usage = proc.cpu_usage();
                stats.cpu_time = Duration::from_millis(proc.accumulated_cpu_time());
                stats.mem_rss = proc.memory();
                stats.mem_vsz = proc.virtual_memory();

                let disk_usage = proc.disk_usage();
                if let Some(interval) = uptime
                    .and_then(|new_uptime| stats.uptime.map(|old_uptime| new_uptime - old_uptime))
                    && !interval.is_zero()
                {
                    let interval = interval.as_secs_f64();
                    stats.io_read = ((disk_usage.total_read_bytes - stats.total_io_read) as f64
                        / interval)
                        .round() as u64;
                    stats.io_write =
                        ((disk_usage.total_written_bytes - stats.total_io_write) as f64 / interval)
                            .round() as u64;
                }

                stats.total_io_read = disk_usage.total_read_bytes;
                stats.total_io_write = disk_usage.total_written_bytes;
                stats.uptime = uptime;

                srv.update_stats(stats);
            } else {
                let stats = srv.stats();
                if stats.uptime.is_some() {
                    srv.update_stats(Stats::default());
                }

                if info.pid.is_some() {
                    srv.set_crashed();
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
    use crate::service::{Command, Status};
    use anyhow::Result;
    use serial_test::serial;

    use super::*;

    #[test]
    #[serial(waitpid)] // raises SIGCHLD
    fn update_empty() -> Result<()> {
        let mut sysinfo = Sysinfo::default();
        let services = DashMap::<ServiceId, Arc<Service>>::new();
        {
            let srv = Service::new("test", Command::new("sleep", ["300"]));
            srv.start();
            services.insert(0, srv.into());
        }
        sysinfo.update(&services);
        assert_eq!(1, sysinfo.pids.len());
        assert_eq!(1, sysinfo.system.processes().len());
        for service in &services {
            let pid = service.info().pid.expect("process should be running");
            service.stop();
            // fake a crash
            service.set_running(pid);
        }

        sysinfo.update(&services);
        assert_eq!(0, sysinfo.pids.len());
        assert_eq!(0, sysinfo.system.processes().len());
        for service in &services {
            assert_eq!(Status::Crashed, service.info().status);
        }

        Ok(())
    }
}
