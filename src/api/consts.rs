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
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
};

pub const DEFAULT_SERVER_ADDR: SocketAddr =
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 5000);

pub fn get_server_addr() -> SocketAddr {
    std::env::var("PPM_LISTEN")
        .ok()
        .and_then(|x| x.parse::<SocketAddr>().ok())
        .unwrap_or(DEFAULT_SERVER_ADDR)
}

pub fn get_forward_socket_path() -> PathBuf {
    match dirs::runtime_dir().or(dirs::cache_dir()) {
        Some(runtime) => runtime.join("ppm-web.unix"),
        None => PathBuf::from("/var/run/ppm-web.unix"),
    }
}
