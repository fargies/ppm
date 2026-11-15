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
** Author: Sylvain Fargier <fargier.sylvain@gmail.com>
*/

use std::{
    collections::HashMap,
    fmt::Debug,
    os::fd::{AsRawFd, OwnedFd, RawFd},
    sync::{
        Arc, Weak,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
};

use anyhow::{Context, Result, anyhow};
use dashmap::DashMap;

use super::Monitor;

use crate::{
    service::ServiceId,
    utils::{
        debug::DebugIter,
        libc::Dup,
        poller::{Poller, PollerFds, PollerFlags, PollerWord, PollerWriter},
    },
};

mod watch;
pub use watch::PortWatch;

type WatchMap = Arc<DashMap<ServiceId, Arc<PortListenerInfo>>>;

/// Xinetd like port watcher
///
/// Forwards the listening socket as child's process stdin
pub struct PortListener {
    poller: PollerWriter,
    join_handle: Option<JoinHandle<()>>,
    watchs: WatchMap,
}

struct PortListenerThreadContext {
    watchs: WatchMap,
    poller: Poller,
    monitor: Weak<Monitor>,
}

impl PortListenerThreadContext {
    pub fn run(&mut self) -> Result<()> {
        let mut pfds = self.poller.make_pfds(self.watchs.len().min(1));
        let mut revmap = HashMap::new();
        let mut update_pfds = true;

        loop {
            let _span = tracing::info_span!(parent: None, "port_listener").entered();

            if update_pfds {
                self.prepare(&mut pfds, &mut revmap);
                update_pfds = false;
            }

            let wake_word = self
                .poller
                .poll(&mut pfds, None)
                .context("failed to poll")?;
            tracing::trace!(?wake_word, events = ?DebugIter::new(pfds.iter()), "listener awaken");

            for (fd, flags) in pfds.iter() {
                update_pfds = true;
                if flags.contains(PollerFlags::IN) {
                    match revmap.get(&fd) {
                        Some(port_info) if port_info.is_enabled() => {
                            port_info.process(&self.monitor()?);
                        }
                        None => {
                            tracing::error!(fd, "unknown port-listener fd");
                        }
                        _ => (),
                    }
                } else {
                    tracing::error!(?flags, fd, "listener polling error");
                }
            }

            match wake_word {
                Some(PollerWord::Wake) => {
                    update_pfds = true;
                    tracing::trace!("wake-word received");
                }
                Some(PollerWord::Exit) => {
                    tracing::trace!("exit requested");
                    return Ok(());
                }
                Some(PollerWord::Custom(wake_word)) => {
                    tracing::error!(wake_word, "unknown wake_word received")
                }
                None => (),
            }
        }
    }

    fn prepare(
        &mut self,
        pfds: &mut PollerFds,
        revmap: &mut HashMap<RawFd, Arc<PortListenerInfo>>,
    ) {
        pfds.clear();
        revmap.clear();

        for watch in self.watchs.iter().filter(|w| w.is_enabled()) {
            pfds.push(
                &watch.listener,
                PollerFlags::IN | PollerFlags::ERR | PollerFlags::NVAL,
            );
            revmap.insert(watch.listener.as_raw_fd(), watch.clone());
        }
    }

    fn monitor(&self) -> Result<Arc<Monitor>> {
        self.monitor
            .upgrade()
            .ok_or_else(|| anyhow!("monitor has been released"))
    }
}

impl PortListener {
    pub fn new(monitor: Weak<Monitor>) -> Box<Self> {
        let (poller, poller_writer) = Poller::with_pipe();
        let mut ret = Self {
            poller: poller_writer,
            join_handle: None,
            watchs: Arc::new(DashMap::new()),
        };

        let join_handle = {
            let mut ctx = PortListenerThreadContext {
                watchs: ret.watchs.clone(),
                poller,
                monitor,
            };
            std::thread::spawn(move || {
                ctx.run()
                    .inspect_err(|err| tracing::error!(?err, "listener thread error"))
                    .unwrap_or_default()
            })
        };
        ret.join_handle = Some(join_handle);
        Box::new(ret)
    }

    pub fn add(&mut self, service_id: &ServiceId, watch: &PortWatch) -> Result<()> {
        self.watchs.insert(
            *service_id,
            Arc::new(PortListenerInfo::new(service_id, watch)?),
        );
        self.wake();
        Ok(())
    }

    pub fn remove(&mut self, service_id: &ServiceId) {
        if let Some((_, info)) = self.watchs.remove(service_id) {
            /* info is in an arc and may still emit events from [PortListener] thread */
            info.set_enabled(false);
        }
        self.wake();
    }

    pub fn set_enabled(&mut self, service_id: &ServiceId, value: bool) {
        if let Some(info) = self.watchs.get(service_id).map(|v| Arc::clone(&v)) {
            info.set_enabled(value);
            if value {
                self.wake();
            }
        }
    }

    pub fn has_watch(&self, service_id: &ServiceId) -> bool {
        self.watchs.contains_key(service_id)
    }

    pub fn wake(&mut self) {
        self.poller.wake();
    }

    pub fn socket_dup(&self, service_id: &ServiceId) -> Option<OwnedFd> {
        self.watchs
            .get(service_id)
            .and_then(|v| v.listener.dup().ok())
    }
}

impl Debug for PortListener {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PortListener")
            .field(
                "services",
                &DebugIter::new(self.watchs.iter().map(|x| *x.key())),
            )
            .finish()
    }
}

struct PortListenerInfo {
    pub service_id: ServiceId,
    pub enabled: AtomicBool,
    pub listener: Arc<OwnedFd>,
}

impl PortListenerInfo {
    pub fn new(sid: &ServiceId, watch: &PortWatch) -> Result<Self> {
        Ok(Self {
            service_id: *sid,
            enabled: AtomicBool::from(true),
            listener: Arc::new(watch.make_listener()?),
        })
    }

    #[inline]
    pub fn set_enabled(&self, value: bool) {
        self.enabled.store(value, Ordering::Relaxed);
    }

    #[inline]
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    /// returns true if this fd needs to be removed from listener
    pub fn process(&self, monitor: &Arc<Monitor>) {
        /* processing disables the watch, will loop otherwise */
        self.set_enabled(false);
        let service = match monitor.get(&self.service_id) {
            Some(service) => service,
            None => {
                let _span = tracing::info_span!("event", id = self.service_id).entered();
                tracing::error!("no such service");
                return;
            }
        };

        let _span = tracing::info_span!("event", name = service.name, id = service.id).entered();
        tracing::info!("incoming network event");
        monitor.on_portwatch_event(&service);
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io::Read,
        net::{SocketAddr, TcpStream},
        str::FromStr,
        time::Duration,
    };

    use crate::{
        service::{Command, Service, Status},
        utils::{kill_on_drop, wait_for},
    };
    use serde_yaml_ng as yaml;
    use serial_test::serial;

    use super::*;

    fn test_client(message: &str) -> Result<()> {
        let mut stream = TcpStream::connect_timeout(
            &SocketAddr::from_str("127.0.0.1:12345")?,
            Duration::from_millis(100),
        )?;
        tracing::trace!(addr = ?stream.local_addr()?, "client connected");

        let mut value = String::new();
        stream.read_to_string(&mut value)?;
        anyhow::ensure!(value.as_str() == message, "invalid reply message: {value}");
        Ok(())
    }

    const TCP_SERVICE_ARGS: &[&str] = &["run", "-q", "--example", "tcp_service", "--"];

    #[test]
    #[serial(waitpid)]
    fn port_watch() -> Result<()> {
        let mon = Monitor::new();
        let service = {
            let mut srv = Service::new(
                "test",
                Command::new(
                    "cargo",
                    [TCP_SERVICE_ARGS, &["--reply=reply_message"]].concat(),
                ),
            );
            srv.listen = Some(yaml::from_str("127.0.0.1:12345")?);
            mon.insert(srv)
        };

        let join_handle = {
            /* Monitor is handling dead processes */
            let mon = mon.clone();
            std::thread::spawn(move || mon.run())
        };
        let _drop_guard = kill_on_drop(join_handle);
        wait_for!(mon.is_running()).expect("monitor not running");

        for idx in 1..=3 {
            test_client("reply_message")?;

            wait_for!(service.info().restarts == idx).expect("not started");
            wait_for!(service.info().status == Status::Finished).expect("not terminated");
        }
        Ok(())
    }

    #[test]
    #[serial(waitpid)]
    fn port_watch_loop() -> Result<()> {
        let mon = Monitor::new();
        let service = {
            let mut srv = Service::new(
                "test",
                Command::new(
                    "cargo",
                    [TCP_SERVICE_ARGS, &["--reply=reply_message", "--forever"]].concat(),
                ),
            );
            srv.listen = Some(yaml::from_str("127.0.0.1:12345")?);
            mon.insert(srv)
        };

        let join_handle = {
            /* Monitor is handling dead processes */
            let mon = mon.clone();
            std::thread::spawn(move || mon.run())
        };
        let _drop_guard = kill_on_drop(join_handle);
        wait_for!(mon.is_running()).expect("monitor not running");

        test_client("reply_message")?;
        test_client("reply_message")?;
        wait_for!(service.info().restarts == 1).expect("not started");
        mon.stop(&service);

        test_client("reply_message").expect_err("connection should be rejected");

        Ok(())
    }

    #[test]
    #[serial(waitpid)]
    fn port_watch_reschedule() -> Result<()> {
        let mon = Monitor::new();
        let service = {
            let mut srv = Service::new(
                "test",
                Command::new(
                    "cargo",
                    [TCP_SERVICE_ARGS, &["--reply=reply_message"]].concat(),
                ),
            );
            srv.listen = Some(yaml::from_str("127.0.0.1:12345")?);
            mon.insert(srv)
        };

        let join_handle = {
            /* Monitor is handling dead processes */
            let mon = mon.clone();
            std::thread::spawn(move || mon.run())
        };
        let _drop_guard = kill_on_drop(join_handle);
        wait_for!(mon.is_running()).expect("monitor not running");

        test_client("reply_message")?;
        wait_for!(service.info().restarts == 1).expect("not started");
        wait_for!(service.info().status == Status::Finished).expect("not terminated");

        mon.stop(&service);

        test_client("reply_message").expect_err("connection should be rejected");

        /* reschedule should restart the listener */
        mon.reschedule(&service);

        test_client("reply_message")?;
        wait_for!(service.info().restarts == 2).expect("not started");
        wait_for!(service.info().status == Status::Finished).expect("not terminated");

        Ok(())
    }
}
