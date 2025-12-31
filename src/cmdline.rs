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
** Created on: 2025-12-24T14:29:00
** Author: Sylvain Fargier <fargier.sylvain@gmail.com>
*/

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use clap::{CommandFactory, Parser, Subcommand};
use serde::{Deserialize, Serialize};

mod client;
pub use client::Client;

mod server;
pub use server::Server;

pub const DEFAULT_ADDR: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 5000);

#[derive(Serialize, Deserialize, Subcommand, Debug)]
pub enum Action {
    /// Start the daemon
    Daemon,
    /// Start the daemon
    #[command(skip)]
    List,
    /// Dump info
    Info,
}

impl Default for Action {
    fn default() -> Self {
        Action::Daemon {}
    }
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Args {
    #[command(subcommand)]
    pub action: Action,
    #[arg(long, global = true, default_value_t = DEFAULT_ADDR)]
    pub addr: SocketAddr,
}
