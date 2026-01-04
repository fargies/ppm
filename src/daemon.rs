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
use dirs::{config_local_dir, home_dir};
use std::{env::current_dir, net::SocketAddr, path::PathBuf, sync::Arc};

use cmdline::DEFAULT_ADDR;
use monitor::Monitor;
use tracing_subscriber::{
    EnvFilter, Registry, filter::LevelFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt,
};

pub mod cmdline;
pub mod monitor;
pub mod service;

mod utils;
use utils::LoadFromFile;
use utils::signal;

#[tracing::instrument(ret)]
pub fn find_config_file() -> Option<PathBuf> {
    let confdir_file = ["partner", "partner-pm.yml"].iter().collect::<PathBuf>();
    config_local_dir()
        .map(|f| f.join(confdir_file))
        .filter(|f| f.exists())
        .or_else(|| {
            [home_dir(), current_dir().ok()]
                .into_iter()
                .filter_map(|f| f.map(|f| f.join(".partner-pm.yml")))
                .find(|f| f.exists())
        })
}

fn main() -> Result<()> {
    Registry::default()
        .with(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .with(fmt::layer())
        .init();

    let addr = std::env::var("PPM_LISTEN")
        .ok()
        .and_then(|x| x.parse::<SocketAddr>().ok())
        .unwrap_or(DEFAULT_ADDR);

    // block signal before spawning threads to apply mask to all threads
    (signal::SignalSet::default() + signal::SIGALRM + signal::SIGCHLD + signal::SIGTERM).block()?;

    tracing::trace!("starting daemon");
    let monitor = Arc::new(
        find_config_file()
            .and_then(|filename| Monitor::load_from_file(&filename).ok())
            .unwrap_or_default(),
    );
    let server = cmdline::Server::new(Arc::clone(&monitor), addr)?;

    std::thread::spawn(move || server.run());
    monitor.run()
}
