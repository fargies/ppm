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
** Created on: 2025-12-22T15:46:40
** Author: Sylvain Fargier <fargier.sylvain@gmail.com>
*/

use anyhow::Result;
use std::{net::SocketAddr, sync::Arc};

use cmdline::DEFAULT_ADDR;
use monitor::Monitor;
// use tracing_forest::{ForestLayer};
use tracing_subscriber::{fmt, EnvFilter, Registry, layer::SubscriberExt, util::SubscriberInitExt};

pub mod cmdline;
pub mod monitor;
pub mod service;

mod utils;
use utils::signal;

fn main() -> Result<()> {
    Registry::default()
        .with(EnvFilter::from_default_env())
        .with(fmt::layer())
        .init();

    let addr = std::env::var("PPM_LISTEN")
        .ok()
        .and_then(|x| x.parse::<SocketAddr>().ok())
        .unwrap_or(DEFAULT_ADDR);

    // block signal before spawning threads to apply mask to all threads
    (signal::SignalSet::default() + signal::SIGALRM + signal::SIGCHLD + signal::SIGTERM).block()?;

    tracing::trace!("starting daemon");
    let monitor = Arc::new(Monitor::default());
    let server = cmdline::Server::new(Arc::clone(&monitor), addr)?;

    std::thread::spawn(move || server.run());
    monitor.run()
}
