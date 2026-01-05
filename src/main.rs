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
use clap::Parser;
use cmdline::{Action, Args, Client};
use std::{env::current_exe, os::unix::process::CommandExt, path::Path, process};
use tracing_subscriber::{EnvFilter, Registry, fmt, layer::SubscriberExt, util::SubscriberInitExt};

pub mod cmdline;
pub mod monitor;
pub mod service;
pub mod utils;

fn main() -> Result<()> {
    Registry::default()
        .with(EnvFilter::from_default_env())
        .with(fmt::layer().with_writer(std::io::stderr))
        .init();

    let args = Args::parse();
    match args.action {
        // `exec` the daemon process
        Action::Daemon { config } => Err(process::Command::new(
            current_exe()?
                .parent()
                .unwrap_or(Path::new("/"))
                .join("ppm-daemon"),
        )
        .env("PPM_CONFIG", config.unwrap_or_default())
        .exec())?,
        action => {
            Client::connect(args.addr)?.run(&action)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[ctor::ctor]
    fn log_init() {
        Registry::default()
            .with(EnvFilter::from_default_env())
            .with(tracing_forest::ForestLayer::default())
            .try_init();
    }
}
