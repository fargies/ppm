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
** Created on: 2025-12-24T14:34:11
** Author: Sylvain Fargier <fargier.sylvain@gmail.com>
*/

use anyhow::{Context, Result, anyhow};
use colored::Colorize;
use serde::de::DeserializeOwned;
use std::{
    collections::HashMap,
    io::BufReader,
    net::{TcpStream, ToSocketAddrs},
    time::{Duration, Instant},
};
use tabled::{
    Table, Tabled,
    derive::display,
    settings::{
        Alignment, Color, Style,
        object::{Rows, Segment},
        style::BorderColor,
    },
};

use crate::{
    monitor::scheduler::SchedulerEvent,
    service::{self, ServiceId},
    utils::{self, IS_OUT_COLORED, serializers::tabled::TDisplay},
};

use super::{Action, ActionResult};

const STATS_DAEMON_NAME: &str = "<PPM daemon>";

#[derive(Debug)]
pub struct Client(TcpStream);

impl Client {
    #[tracing::instrument(level = "TRACE", skip(addr), ret, err)]
    pub fn connect<A: ToSocketAddrs>(addr: A) -> Result<Client> {
        let stream = TcpStream::connect(addr).context("failed to connect daemon")?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        Ok(Self(stream))
    }
}

impl Client {
    #[tracing::instrument(skip(self), ret(level = "TRACE"), err)]
    /// Invoke a single action
    pub fn invoke<R>(&self, action: &Action) -> Result<R>
    where
        R: DeserializeOwned + std::fmt::Debug,
    {
        let mut reader = serde_json::Deserializer::from_reader(BufReader::new(&self.0))
            .into_iter::<ActionResult<R>>();
        serde_json::to_writer(&self.0, &action)?;
        tracing::trace!("action sent");

        if let Some(reply) = reader.next() {
            reply.context("no reply from daemon")?.into()
        } else {
            Err(anyhow!("empty reply from daemon"))
        }
    }

    #[tracing::instrument(fields(client = ?self.0.local_addr()?, server = ?self.0.peer_addr()?, ?action), skip(self))]
    /// Run a complete action, displaying result on the console
    pub fn run(&self, action: &Action) -> Result<()> {
        match action {
            Action::Daemon { .. } => unimplemented!("must be handled before connecting"),
            Action::List | Action::DaemonStats => unimplemented!("not available from cmdline"),
            Action::Info => {
                let services_list: HashMap<ServiceId, String> = self.invoke(&Action::List)?;
                let info: HashMap<ServiceId, service::Info> = self.invoke(&Action::Info)?;
                let mut keys: Vec<ServiceId> = info.keys().copied().collect();
                keys.sort();

                let data = keys
                    .iter()
                    .filter_map(|id| info.get(id).map(|x| (id, x)))
                    .map(|(id, info)| InfoRecord {
                        id: *id,
                        name: services_list.get(id),
                        info,
                    });
                self.display(Table::new(data));
                Ok(())
            }
            action @ Action::Stats { .. } => {
                let services_list: HashMap<ServiceId, String> = self.invoke(&Action::List)?;
                let stats: HashMap<ServiceId, service::Stats> = self.invoke(action)?;
                let daemon_stats: service::Stats = self.invoke(&Action::DaemonStats)?;
                let daemon_name = String::from(STATS_DAEMON_NAME);
                let mut keys: Vec<ServiceId> = stats.keys().copied().collect();
                keys.sort();

                let data = Some(StatsRecord {
                    id: 0,
                    name: Some(&daemon_name),
                    stats: daemon_stats.uptime.map(|_| &daemon_stats),
                })
                .into_iter()
                .chain(
                    keys.iter()
                        .filter_map(|id| stats.get(id).map(|x| (id, x)))
                        .map(|(id, stats)| StatsRecord {
                            id: *id,
                            name: services_list.get(id),
                            stats: stats.uptime.map(|_| stats),
                        }),
                );
                self.display(Table::new(data));
                Ok(())
            }
            action @ (Action::Stop { .. } | Action::Restart { .. } | Action::Remove { .. }) => {
                self.0.set_read_timeout(Some(Duration::from_secs(30)))?;
                self.invoke(action)
            }
            action @ Action::ShowConfiguration => self
                .invoke::<String>(action)
                .map(|config| print!("{config}")),
            action @ Action::ShowScheduler => {
                let services_list: HashMap<ServiceId, String> = self.invoke(&Action::List)?;
                let sched: Vec<SchedulerEvent> = self.invoke(action)?;
                let data = sched
                    .into_iter()
                    .map(|event| SchedulerEventRecord::new(event, &services_list));
                self.display(Table::new(data));
                Ok(())
            }
            action => self.invoke(action),
        }
    }

    fn display(&self, mut table: Table) {
        table
            .with(Style::rounded().remove_horizontals())
            .with(Alignment::center());

        if utils::IS_OUT_COLORED.get() {
            table
                .modify(Rows::first(), Color::FG_BRIGHT_BLUE)
                .modify(Segment::all(), BorderColor::filled(Color::FG_BRIGHT_BLUE));
        }

        println!("{}", table);
    }
}

#[derive(Tabled)]
struct InfoRecord<'a, 'b> {
    id: ServiceId,
    #[tabled(display("display::option", ""))]
    name: Option<&'a String>,
    #[tabled(inline)]
    info: &'b service::Info,
}

#[derive(Tabled)]
struct StatsRecord<'a, 'b> {
    #[tabled(display("stats_id", self))]
    id: ServiceId,
    #[tabled(display("stats_name", self))]
    name: Option<&'a String>,
    #[tabled(inline)]
    stats: Option<&'b service::Stats>,
}

fn stats_id(id: &ServiceId, rec: &StatsRecord) -> String {
    if rec.name.is_some_and(|name| name == STATS_DAEMON_NAME) {
        String::new()
    } else if rec.stats.is_none() && IS_OUT_COLORED.get() {
        id.to_string().bright_black().to_string()
    } else {
        id.to_string()
    }
}

fn stats_name(name: &Option<&String>, rec: &StatsRecord) -> String {
    if let Some(name) = name {
        if (rec.stats.is_none() || name.as_str() == STATS_DAEMON_NAME) && IS_OUT_COLORED.get() {
            name.bright_black().to_string()
        } else {
            name.to_string()
        }
    } else {
        String::new()
    }
}

#[derive(Tabled)]
struct SchedulerEventRecord<'a> {
    #[tabled(display("display::option", ""))]
    id: Option<ServiceId>,
    #[tabled(display("display::option", ""))]
    name: Option<&'a String>,
    event: String,
    #[tabled(display("TDisplay::to_string"), rename = "scheduled time")]
    timestamp: Instant,
}

impl<'a> SchedulerEventRecord<'a> {
    pub fn new(event: SchedulerEvent, services_list: &'a HashMap<ServiceId, String>) -> Self {
        let id = event.id();
        Self {
            id,
            name: id.and_then(|id| services_list.get(&id)),
            event: Self::event_name(&event),
            timestamp: *event.instant(),
        }
    }

    fn event_name(event: &SchedulerEvent) -> String {
        match event {
            SchedulerEvent::ServiceSchedule { .. } => "schedule",
            SchedulerEvent::ServiceRestart { .. } => "restart",
            SchedulerEvent::Sysinfo { .. } => "stats",
        }
        .into()
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use std::net::TcpListener;

    use super::*;

    #[test]
    fn client() -> Result<()> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let addr = listener.local_addr().unwrap();

        let cli = Client::connect(addr)?;
        cli.0.set_read_timeout(Some(Duration::from_secs(1)))?;

        assert!(
            cli.run(&Action::Info {})
                .unwrap_err()
                .to_string()
                .contains("no reply")
        );
        Ok(())
    }
}
