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

use std::{
    marker::PhantomData,
    net::{IpAddr, Ipv4Addr, SocketAddr},
};

use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize, de::Visitor, ser::SerializeStruct};

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
    /// Dump info (aliases: list)
    #[clap(alias = "list")]
    Info,
    /// Restart the given service (aliases: start)
    #[clap(alias = "start")]
    Restart { service: String },
    /// Stop (terminate) the given service (aliases: terminate)
    #[clap(alias = "terminate")]
    Stop { service: String },
}

impl Default for Action {
    fn default() -> Self {
        Action::Daemon {}
    }
}

#[derive(Debug)]
pub enum ActionResult<T> {
    Ok(T),
    Err(String),
}

impl<T> Serialize for ActionResult<T>
where
    T: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            ActionResult::Ok(result) => {
                let mut ret = serializer.serialize_struct("result", 1)?;
                ret.serialize_field("result", result)?;
                ret.end()
            }
            ActionResult::Err(msg) => {
                let mut ret = serializer.serialize_struct("error", 1)?;
                ret.serialize_field("error", msg)?;
                ret.end()
            }
        }
    }
}

impl<'de, T> Deserialize<'de> for ActionResult<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct ActionResultVisitor<T>(PhantomData<T>);
        impl<'de, T> Visitor<'de> for ActionResultVisitor<T>
        where
            T: Deserialize<'de>,
        {
            type Value = ActionResult<T>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str(
                    "expecting a `{ \"result\": true }` or `{ \"error\": \"msg\" } object",
                )
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                use serde::de::Error;

                while let Some(k) = map.next_key::<String>()? {
                    if k == "result" {
                        return Ok(ActionResult::Ok(map.next_value::<T>()?));
                    } else if k == "error" {
                        return Ok(ActionResult::Err(map.next_value::<String>()?));
                    }
                }
                Err(Error::custom("no \"result\" or \"error\" fields found"))
            }
        }

        deserializer.deserialize_map(ActionResultVisitor(PhantomData))
    }
}

impl<T> From<anyhow::Result<T>> for ActionResult<T> {
    fn from(value: anyhow::Result<T>) -> Self {
        match value {
            Ok(ret) => ActionResult::Ok(ret),
            Err(err) => ActionResult::Err(err.to_string()),
        }
    }
}

impl<T> From<anyhow::Error> for ActionResult<T> {
    fn from(value: anyhow::Error) -> Self {
        ActionResult::Err(value.to_string())
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

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::{Result, anyhow};

    #[test]
    fn action_result() -> Result<()> {
        assert_eq!(
            "{\"result\":true}",
            serde_json::to_string(&ActionResult::Ok(true))?.as_str()
        );

        assert_eq!(
            "{\"error\":\"critical failure\"}",
            serde_json::to_string(&ActionResult::<()>::Err("critical failure".into()))?.as_str()
        );

        assert_eq!(
            "{\"error\":\"critical failure\"}",
            serde_json::to_string(&ActionResult::<()>::from(anyhow!("critical failure")))?.as_str()
        );
        Ok(())
    }
}
