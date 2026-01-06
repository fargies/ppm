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
** Created on: 2025-12-24T16:24:16
** Author: Sylvain Fargier <fargier.sylvain@gmail.com>
*/

use anyhow::{Context, Result};
use serde_yaml_ng as yaml;
use std::{
    io::BufReader,
    net::{Shutdown, TcpListener, TcpStream, ToSocketAddrs},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use crate::{
    monitor::Monitor,
    service::{Command, Service, ServiceId},
    utils::{InnerRef, wrap_map_iterator},
};

use super::{Action, ActionResult};

const MAX_CONNECTIONS: usize = 16;

#[derive(Debug)]
pub struct Server {
    pub socket: TcpListener,
    pub monitor: Arc<Monitor>,
    pub connections: Arc<AtomicUsize>,
}

pub struct ServerToken(Arc<AtomicUsize>);

impl ServerToken {
    pub fn new(counter: &Arc<AtomicUsize>) -> (Self, usize) {
        (
            Self(Arc::clone(counter)),
            counter.fetch_add(1, Ordering::Relaxed),
        )
    }
}

impl Drop for ServerToken {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}

impl Server {
    #[tracing::instrument(skip(addr, monitor))]
    pub fn new<A: ToSocketAddrs>(monitor: Arc<Monitor>, addr: A) -> Result<Server> {
        let ret = Self {
            socket: TcpListener::bind(addr).context("failed to listen")?,
            monitor,
            connections: AtomicUsize::new(0).into(),
        };
        tracing::info!(addr = ?ret.socket.local_addr()?, "listening");
        Ok(ret)
    }

    #[tracing::instrument(skip(self))]
    pub fn run(&self) {
        loop {
            let _span = tracing::info_span!(parent: None, "server").entered();

            match self.socket.accept() {
                Ok(stream) => {
                    let (token, count) = ServerToken::new(&self.connections);
                    if count > MAX_CONNECTIONS {
                        tracing::error!(client = ?stream.1, "connection rejected");
                        if let Err(error) = stream.0.shutdown(Shutdown::Both) {
                            tracing::error!(client = ?stream.1, ?error, "shutdown failed");
                        }
                    } else {
                        let monitor = Arc::clone(&self.monitor);
                        std::thread::spawn(move || {
                            if let Err(error) = Server::handle(&stream.0, monitor) {
                                let _ = serde_json::to_writer(
                                    &stream.0,
                                    &ActionResult::<()>::from(error),
                                );
                            }
                            drop(token);
                        });
                    }
                }
                Err(error) => {
                    tracing::error!(?error, "socket error");
                    break;
                }
            }
        }
    }

    fn find_service(monitor: &Monitor, service: &String) -> Option<Arc<Service>> {
        service
            .parse::<ServiceId>()
            .ok()
            .and_then(|id| monitor.get(id))
            .or_else(|| monitor.find_by_name(service))
    }

    #[tracing::instrument(fields(client = ?stream.peer_addr()?), skip(stream, monitor), err)]
    fn handle(stream: &TcpStream, monitor: Arc<Monitor>) -> Result<()> {
        let mut reader =
            serde_json::Deserializer::from_reader(BufReader::new(stream)).into_iter::<Action>();

        while let Some(Ok(action)) = reader.next() {
            tracing::trace!(?action, "action requested");
            if let Err(e) = Server::run_action(stream, &monitor, action) {
                serde_json::to_writer(stream, &ActionResult::<()>::from(e))?;
            }
        }
        Ok(())
    }

    fn run_action(stream: &TcpStream, monitor: &Monitor, action: Action) -> Result<()> {
        match action {
            Action::Daemon { .. } => {
                unimplemented!("daemon command must be handled from client side (fork/exec)")
            }
            Action::List => serde_json::to_writer(
                stream,
                &wrap_map_iterator(
                    monitor
                        .services
                        .iter()
                        .map(|x| (x.id, InnerRef(x, |x| &x.name))),
                ),
            )?,
            Action::Info => serde_json::to_writer(
                stream,
                &wrap_map_iterator(monitor.services.iter().map(|x| (x.id, x.info()))),
            )?,
            Action::Stats { service } => {
                if let Some(service) = service {
                    let service = Server::find_service(monitor, &service)
                        .with_context(|| format!("no such service \"{service}\""))?;
                    serde_json::to_writer(
                        stream,
                        &wrap_map_iterator([(service.id, service.stats())].into_iter()),
                    )?
                } else {
                    serde_json::to_writer(
                        stream,
                        &wrap_map_iterator(monitor.services.iter().map(|x| (x.id, x.stats()))),
                    )?
                }
            }
            Action::DaemonStats => serde_json::to_writer(stream, &monitor.stats())?,
            Action::Restart { service } => {
                let service = Server::find_service(monitor, &service)
                    .with_context(|| format!("no such service \"{service}\""))?;

                service.restart();
                serde_json::to_writer(stream, &ActionResult::Ok(()))?;
            }
            Action::Stop { service } => {
                let service = Server::find_service(monitor, &service)
                    .with_context(|| format!("no such service \"{service}\""))?;
                service.stop();
                serde_json::to_writer(stream, &ActionResult::Ok(()))?;
            }
            Action::ShowConfiguration => {
                serde_json::to_writer(stream, &ActionResult::Ok(yaml::to_string(&monitor)?))?;
            }
            Action::Add { name, env, command } => {
                let mut args = command.into_iter();
                let path = args.next().context("command is empty")?;

                let mut service = Service::new(name, Command::new(path, args));
                if !env.is_empty() {
                    service.command.env = Some(env.into_iter().collect());
                }

                service.start();
                monitor.insert(service);
                serde_json::to_writer(stream, &ActionResult::Ok(()))?;
            }
            Action::Remove { service } => {
                let service = Server::find_service(monitor, &service)
                    .with_context(|| format!("no such service \"{service}\""))?;
                service.stop();
                monitor.services.remove(&service.id);
                serde_json::to_writer(stream, &ActionResult::Ok(()))?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        cmdline::Client,
        service::{Command, Service},
    };

    use super::*;
    use anyhow::Result;

    #[test]
    fn request() -> Result<()> {
        let monitor = Monitor::default();
        monitor.insert(Service::new("test", Command::new("ls", ["-la"])));
        let server = Server::new(monitor.into(), "127.0.0.1:0")?;
        let addr = server.socket.local_addr()?;

        std::thread::spawn(move || server.run());

        let client = Client::connect(addr)?;
        client.run(&Action::Info).expect("command failed");
        Ok(())
    }
}
