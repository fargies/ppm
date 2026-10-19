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

use anyhow::{Context, Result, anyhow};
use serde_yaml_ng as yaml;
use std::{
    io::BufReader,
    net::{Shutdown, TcpListener, TcpStream, ToSocketAddrs},
    os::fd::{FromRawFd, IntoRawFd},
    path::PathBuf,
    sync::{
        Arc, LazyLock,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use crate::{
    monitor::{
        Monitor,
        logger::{LOGGER_DAEMON_ID, LOGGER_DAEMON_NAME},
        scheduler::SchedulerEvent,
    },
    service::{Command, Service, ServiceId},
    utils::{InnerRef, find_ppm_tool, wrap_map_iterator},
};

use crate::api::{Action, ActionResult, Reply, Request, RequestId};

const MAX_CONNECTIONS: usize = 16;
static PPM_CLI_EXE: LazyLock<Option<PathBuf>> = LazyLock::new(|| find_ppm_tool("ppm"));

#[derive(Debug)]
pub struct Server {
    pub socket: TcpListener,
    pub monitor: Arc<Monitor>,
    pub connections: Arc<AtomicUsize>,
    running: AtomicBool,
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
            running: AtomicBool::new(false),
        };
        tracing::info!(addr = ?ret.socket.local_addr()?, "listening");
        Ok(ret)
    }

    #[tracing::instrument(skip(self), ret, err)]
    pub fn run(&self) -> Result<()> {
        loop {
            let _span = tracing::info_span!(parent: None, "server:run").entered();
            self.running.store(true, Ordering::Relaxed);

            match self.socket.accept() {
                Ok(stream) => {
                    tracing::trace!("connection opened");
                    if !self.running.load(Ordering::Relaxed) {
                        tracing::info!("shutdown request");
                        stream.0.shutdown(Shutdown::Both).unwrap_or(());
                        return Ok(());
                    }
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
                            tracing::trace!("connection closed");
                            stream.0.shutdown(Shutdown::Both).unwrap_or(());
                            drop(token);
                        });
                    }
                }
                Err(err) => {
                    if !self.running.fetch_and(false, Ordering::Relaxed) {
                        tracing::trace!(?err, "socket error");
                        return Ok(());
                    }
                    tracing::info!(running = self.running.load(Ordering::Relaxed), "running?");
                    return Err(err.into());
                }
            }
        }
    }

    #[tracing::instrument(skip(self), ret)]
    pub fn stop(&self) -> Result<()> {
        self.running.store(false, Ordering::Relaxed);
        if let Err(err) =
            TcpStream::connect_timeout(&self.socket.local_addr()?, Duration::from_secs(5))
                .and_then(|stream| stream.shutdown(Shutdown::Both))
        {
            tracing::trace!(?err, "server connetion error");
        }
        Ok(())
    }

    fn find_service(monitor: &Monitor, service: &String) -> Option<Arc<Service>> {
        service
            .parse::<ServiceId>()
            .ok()
            .and_then(|id| monitor.get(&id))
            .or_else(|| monitor.find_by_name(service))
    }

    fn is_daemon(service: &String) -> bool {
        match service.parse::<ServiceId>() {
            Ok(id) => id == LOGGER_DAEMON_ID,
            Err(_) => service == LOGGER_DAEMON_NAME,
        }
    }

    #[tracing::instrument(fields(client = ?stream.peer_addr()?), skip(stream, monitor), err)]
    fn handle(stream: &TcpStream, monitor: Arc<Monitor>) -> Result<()> {
        let reader =
            serde_json::Deserializer::from_reader(BufReader::new(stream)).into_iter::<Request>();

        for pkt in reader {
            match pkt {
                Ok(Request { id, action }) => {
                    tracing::trace!(?id, ?action, "action requested");
                    if let Err(e) = Server::run_action(stream, &monitor, &id, action) {
                        serde_json::to_writer(stream, &Reply::<()>::err(&id, e))?;
                    }
                }
                Err(err) => {
                    tracing::warn!(?err, "failed to build request");
                    serde_json::to_writer(stream, &Reply::<()>::err(&None, err))?
                }
            }
        }
        Ok(())
    }

    fn run_action(
        stream: &TcpStream,
        monitor: &Arc<Monitor>,
        req_id: &Option<RequestId>,
        action: Action,
    ) -> Result<()> {
        match action {
            Action::Daemon { .. } => {
                unimplemented!("daemon command must be handled from client side (fork/exec)")
            }
            Action::List => serde_json::to_writer(
                stream,
                &Reply::ok(
                    req_id,
                    wrap_map_iterator(
                        monitor
                            .services
                            .iter()
                            .map(|x| (x.id, InnerRef(x, |x| &x.name))),
                    ),
                ),
            )?,
            Action::Info { service } => match service {
                Some(service) => {
                    let service = Server::find_service(monitor, &service)
                        .with_context(|| format!("no such service \"{service}\""))?;
                    serde_json::to_writer(
                        stream,
                        &Reply::ok(req_id, wrap_map_iterator([(service.id, service.info())])),
                    )?
                }
                None => serde_json::to_writer(
                    stream,
                    &Reply::ok(
                        req_id,
                        wrap_map_iterator(monitor.services.iter().map(|x| (x.id, x.info()))),
                    ),
                )?,
            },
            Action::Stats { service } => {
                if let Some(service) = service {
                    let service = Server::find_service(monitor, &service)
                        .with_context(|| format!("no such service \"{service}\""))?;
                    serde_json::to_writer(
                        stream,
                        &Reply::ok(req_id, wrap_map_iterator([(service.id, service.stats())])),
                    )?
                } else {
                    serde_json::to_writer(
                        stream,
                        &Reply::ok(
                            req_id,
                            wrap_map_iterator(monitor.services.iter().map(|x| (x.id, x.stats()))),
                        ),
                    )?
                }
            }
            Action::DaemonStats => {
                serde_json::to_writer(stream, &Reply::ok(req_id, monitor.stats()))?
            }
            Action::Restart { service } => {
                let service = Server::find_service(monitor, &service)
                    .with_context(|| format!("no such service \"{service}\""))?;

                monitor.restart(&service);
                serde_json::to_writer(stream, &Reply::ok(req_id, ()))?;
            }
            Action::Reschedule { service } => {
                let service = Server::find_service(monitor, &service)
                    .with_context(|| format!("no such service \"{service}\""))?;

                monitor.reschedule(&service);
                serde_json::to_writer(stream, &Reply::ok(req_id, ()))?;
            }
            Action::Stop { service } => {
                let service = Server::find_service(monitor, &service)
                    .with_context(|| format!("no such service \"{service}\""))?;

                monitor.stop(&service);
                serde_json::to_writer(stream, &Reply::ok(req_id, ()))?;
            }
            Action::ShowConfiguration => {
                serde_json::to_writer(stream, &Reply::ok(req_id, yaml::to_string(&monitor)?))?;
            }
            Action::ShowScheduler => {
                serde_json::to_writer(stream, &Reply::ok(req_id, monitor.scheduler.dump()))?;
            }
            Action::ClockCheck => {
                monitor.scheduler.enqueue(SchedulerEvent::ClockCheck {
                    instant: Instant::now(),
                });
                serde_json::to_writer(stream, &Reply::ok(req_id, ()))?
            }
            Action::Add {
                name,
                env,
                command,
                schedule,
                workdir,
            } => {
                let mut args = command.into_iter();
                let path = args.next().context("command is empty")?;

                let mut service = Service::new(name, Command::new(path, args));
                if !env.is_empty() {
                    service.command.env = Some(env.into_iter().collect());
                }
                if let Some(schedule) = schedule {
                    service.schedule = Some(schedule.parse()?);
                }
                service.workdir = workdir;

                monitor.insert(service);
                serde_json::to_writer(stream, &Reply::ok(req_id, ()))?;
            }
            Action::Remove { service } => {
                let service = Server::find_service(monitor, &service)
                    .with_context(|| format!("no such service \"{service}\""))?;
                service.stop();
                monitor.remove(&service.id);
                serde_json::to_writer(stream, &Reply::ok(req_id, ()))?;
            }
            Action::ListLogFiles { service } => {
                let logger = monitor
                    .logger
                    .as_ref()
                    .ok_or(anyhow!("logger not enabled"))?;
                let id = Server::find_service(monitor, &service)
                    .map(|service| service.id)
                    .or_else(|| Server::is_daemon(&service).then_some(LOGGER_DAEMON_ID))
                    .with_context(|| format!("no such service \"{service}\""))?;

                serde_json::to_writer(stream, &Reply::ok(req_id, logger.list_files(id)))?;
            }
            Action::Log {
                service,
                lines,
                follow,
            } => {
                let cli = PPM_CLI_EXE.as_ref().ok_or(anyhow!("ppm cli not found"))?;
                let mut cmd = std::process::Command::new(cli);
                cmd.stdin(std::process::Stdio::null()).args([
                    "log",
                    "--addr",
                    stream.local_addr()?.to_string().as_str(),
                ]);
                if let Some(lines) = lines {
                    cmd.args(["-n", lines.to_string().as_str()]);
                }
                if follow.is_some_and(|f| f) {
                    cmd.arg("-f");
                }
                cmd.arg(service);
                let raw_fd = stream.try_clone()?.into_raw_fd();
                cmd.stdout(unsafe { std::process::Stdio::from_raw_fd(raw_fd) })
                    .stderr(std::process::Stdio::inherit());
                cmd.spawn()?;
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
        utils::OnDrop,
    };

    use super::*;
    use anyhow::Result;
    use serial_test::serial;

    #[test]
    #[serial(waitpid)]
    fn request() -> Result<()> {
        let monitor = Monitor::new();
        monitor.insert(Service::new("test", Command::new("ls", ["-la"])));
        let server = Arc::new(Server::new(monitor, "127.0.0.1:0")?);
        let addr = server.socket.local_addr()?;

        let join_handle = {
            let server = Arc::clone(&server);
            std::thread::spawn(move || server.run())
        };
        let _drop_guard = OnDrop::new(|| {
            server.stop().unwrap();
            join_handle.join().unwrap().unwrap();
        });

        let client = Client::connect(addr)?;
        client
            .run(&Action::Info { service: None })
            .expect("command failed");

        Ok(())
    }
}
