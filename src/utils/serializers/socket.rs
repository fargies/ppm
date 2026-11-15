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
    net::{Ipv4Addr, SocketAddr},
    str::FromStr,
};

use serde::{Deserializer, de::Visitor};

struct SocketVisitor;
impl<'de> Visitor<'de> for SocketVisitor {
    type Value = SocketAddr;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a socket address or port number")
    }

    fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        if v <= u16::MAX as u64 {
            Ok(SocketAddr::from((Ipv4Addr::UNSPECIFIED, v as u16)))
        } else {
            Err(E::custom(format!("port out of range: {v}")))
        }
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        if let Some(port) = v
            .strip_prefix(':')
            .and_then(|value| value.parse::<u16>().ok())
        {
            Ok(SocketAddr::from((Ipv4Addr::UNSPECIFIED, port)))
        } else {
            match SocketAddr::from_str(v) {
                Ok(value) => Ok(value),
                Err(_) if let Ok(value) = v.parse::<u16>() => {
                    Ok(SocketAddr::from((Ipv4Addr::UNSPECIFIED, value)))
                }
                Err(err) => Err(E::custom(format!("{err}: {v}"))),
            }
        }
    }
}

pub fn parse<'de, D>(d: D) -> Result<SocketAddr, D::Error>
where
    D: Deserializer<'de>,
{
    d.deserialize_any(SocketVisitor)
}

#[cfg(test)]
mod tests {
    use std::{net::SocketAddr, str::FromStr};
    use anyhow::Result;
    use serde::{Deserialize, Serialize};
    use serde_yaml_ng as yaml;

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    pub struct SocketParseTest(#[serde(deserialize_with = "super::parse")] pub SocketAddr);

    #[test]
    fn socket_parse_test() -> Result<()> {
        for (input, sock) in [
            ("1234", "0.0.0.0:1234"),
            (":1234", "0.0.0.0:1234"),
            ("\"1234\"", "0.0.0.0:1234"),
            ("127.0.0.1:10000", "127.0.0.1:10000"),
            ("\"[::]:1234\"", "[::]:1234"),
        ] {
            assert_eq!(
                yaml::from_str::<SocketParseTest>(input)
                    .map_err(|err| anyhow::anyhow!("failed to parse {input}: {err:?}"))?
                    .0,
                SocketAddr::from_str(sock)
                    .map_err(|err| anyhow::anyhow!("failed to parse {sock}: {err:?}"))?,
                "failed to parse socket {input} != {sock}"
            );
        }
        Ok(())
    }
}
