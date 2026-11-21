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

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{
    net::{SocketAddr, TcpListener},
    os::fd::OwnedFd,
};

use crate::utils::serializers::socket;

#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
#[serde(untagged)]
pub enum PortWatch {
    /// Port to watch
    SocketAddr(#[serde(deserialize_with = "socket::parse")] SocketAddr),
}

impl From<SocketAddr> for PortWatch {
    fn from(value: SocketAddr) -> Self {
        Self::SocketAddr(value)
    }
}

impl PortWatch {
    pub fn make_listener(&self) -> Result<OwnedFd> {
        match self {
            PortWatch::SocketAddr(socket_addr) => Ok(TcpListener::bind(socket_addr)?.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use super::*;
    use anyhow::Result;
    use serde_yaml_ng as yaml;

    #[test]
    fn serde() -> Result<()> {
        let watch = yaml::from_str::<PortWatch>(":1234")?;
        assert_eq!(
            watch,
            PortWatch::from(SocketAddr::from((Ipv4Addr::UNSPECIFIED, 1234))),
        );
        Ok(())
    }
}
