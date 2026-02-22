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
use std::{
    env::{self, current_dir},
    net::SocketAddr,
    path::PathBuf,
    sync::Arc,
};

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

#[tracing::instrument(ret)]
pub fn find_config_file() -> Option<PathBuf> {
    if let Some(file) = env::var("PPM_CONFIG")
        .ok()
        .filter(|file| !file.is_empty())
        .map(PathBuf::from)
    {
        // we did not check that it exists on purpose, when set it *must* be used
        Some(file)
    } else if let Some(file) = config_local_dir()
        .map(|f| f.join(["partner", "partner-pm.yml"].iter().collect::<PathBuf>()))
        .filter(|f| f.exists())
    {
        Some(file)
    } else {
        [home_dir(), current_dir().ok()]
            .into_iter()
            .filter_map(|f| f.map(|f| f.join(".partner-pm.yml")))
            .find(|f| f.exists())
    }
}

fn main() -> Result<()> {
    #[cfg(all(feature = "size_optim", target_os = "linux"))]
    unsafe {
        // Disable malloc arenas, will slow down the process but be more
        // conservative on memory.
        //
        // By default 64bits systems will reserve 65MB using mmap per thread
        // making VSZ extra-large, by setting `M_ARENA_MAX` to `1` malloc
        // fallsback on lock based `sbrk` implementation (on a single default
        // created arena).
        //
        // See man (3) mallopt
        libc::mallopt(libc::M_ARENA_MAX, 1);
    }
    Registry::default()
        .with(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .with(fmt::layer().with_target(false))
        .init();

    let addr = std::env::var("PPM_LISTEN")
        .ok()
        .and_then(|x| x.parse::<SocketAddr>().ok())
        .unwrap_or(DEFAULT_ADDR);

    Monitor::init()?;

    tracing::info!("starting daemon");
    let monitor = Arc::new(if let Some(file) = find_config_file() {
        Monitor::load_from_file(&file)?
    } else {
        Monitor::default()
    });

    let server = cmdline::Server::new(Arc::clone(&monitor), addr)?;
    std::thread::Builder::new()
        .name("server".into())
        .spawn(move || server.run())?;
    monitor.run()
}
