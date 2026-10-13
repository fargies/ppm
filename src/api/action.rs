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

use std::marker::PhantomData;

use clap::Subcommand;
use serde::{Deserialize, Serialize, de::Visitor, ser::SerializeStruct};

#[derive(Serialize, Deserialize, Subcommand, Debug, Eq, PartialEq)]
#[serde(tag = "method", content = "params", rename_all = "lowercase")]
pub enum Action {
    /// Start the daemon
    Daemon {
        /// Configuration file to load
        #[clap(long)]
        #[serde(default)]
        config: Option<String>,
    },
    /// Start the daemon
    #[command(skip)]
    List,
    /// Dump info
    #[clap(visible_aliases = ["list", "ls", "status"])]
    Info {
        #[serde(default)]
        /// service id or name
        service: Option<String>,
    },
    /// Restart the given service
    #[clap(visible_alias = "start")]
    Restart {
        /// service id or name
        service: String,
    },
    /// Stop (terminate) the given service
    #[clap(visible_alias = "terminate")]
    Stop {
        /// service id or name
        service: String,
    },
    /// Reschedule a service, flag as active and reschedule
    #[clap(visible_alias = "schedule")]
    Reschedule {
        /// service id or name
        service: String,
    },
    /// Dump running configuration
    #[clap(visible_aliases=["show-config", "config"])]
    ShowConfiguration,
    /// Add a new service
    #[clap(visible_aliases=["add-service"])]
    Add {
        /// Service name
        #[clap(long, short)]
        name: String,
        /// Environment variables
        #[clap(long, short, value_name = "NAME=VALUE", value_parser = parse_key_val::<String, String>)]
        env: Vec<(String, String)>,
        /// Service schedule (cron-like)
        #[clap(long, short)]
        #[serde(default)]
        schedule: Option<String>,
        /// Workdir
        #[clap(long, short)]
        #[serde(default)]
        workdir: Option<String>,
        /// Command to run
        #[clap(last = true)]
        command: Vec<String>,
    },
    /// Stop and remove a service
    #[clap(visible_aliases=["rm", "remove-service"])]
    Remove {
        /// service id or name
        service: String,
    },
    /// Get statistics on a service
    #[clap(visible_aliases=["statistics", "details"])]
    Stats {
        /// service id or name
        #[serde(default)]
        service: Option<String>,
    },
    /// Get statsics from the daemon process
    ///
    /// Used by `statistics` command from cli
    #[command(skip)]
    DaemonStats,

    #[command(skip)]
    ListLogFiles { service: String },

    /// Show service logs
    Log {
        /// service id or name to show logs from, use "ppm-daemon" to see PPM Daemon logs
        service: String,

        /// Number of lines to show
        #[clap(long, short = 'n')]
        #[serde(default)]
        lines: Option<usize>,

        /// Wait for new log-lines
        #[clap(long, short, action = clap::ArgAction::SetTrue)]
        #[serde(default)]
        follow: Option<bool>,
    },

    /// Get scheduler info
    ShowScheduler,
}

fn parse_key_val<T, U>(
    s: &str,
) -> Result<(T, U), Box<dyn std::error::Error + Send + Sync + 'static>>
where
    T: std::str::FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
    U: std::str::FromStr,
    U::Err: std::error::Error + Send + Sync + 'static,
{
    let pos = s
        .find('=')
        .ok_or_else(|| format!("invalid KEY=value: no `=` found in `{s}`"))?;
    Ok((s[..pos].parse()?, s[pos + 1..].parse()?))
}

impl Default for Action {
    fn default() -> Self {
        Action::Daemon { config: None }
    }
}

#[derive(Debug)]
pub enum ActionResult<T> {
    Ok(T),
    Err(String),
}

impl<T> ActionResult<T> {
    pub fn map<F, R>(self, f: F) -> ActionResult<R>
    where
        F: FnOnce(T) -> R,
    {
        match self {
            ActionResult::Ok(value) => ActionResult::Ok(f(value)),
            ActionResult::Err(err) => ActionResult::Err(err),
        }
    }
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

impl<T> From<ActionResult<T>> for anyhow::Result<T> {
    fn from(value: ActionResult<T>) -> Self {
        match value {
            ActionResult::Ok(x) => Ok(x),
            ActionResult::Err(msg) => Err(anyhow::anyhow!(msg)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::{Result, anyhow};

    #[test]
    fn action_serde() -> Result<()> {
        /* has no params */
        assert_eq!(
            r#"{"method":"list"}"#,
            serde_json::to_string(&Action::List)?.as_str()
        );
        /* should not care about params */
        assert_eq!(
            serde_json::from_str::<Action>(r#"{"method":"list","params":null}"#)?,
            Action::List
        );
        /* has optional params */
        assert_eq!(
            serde_json::from_str::<Action>(r#"{"method":"info","params":{}}"#)?,
            Action::Info { service: None }
        );
        assert_eq!(
            r#"{"method":"info","params":{"service":null}}"#,
            serde_json::to_string(&Action::Info { service: None })?.as_str()
        );
        assert_eq!(
            r#"{"method":"info","params":{"service":"0"}}"#,
            serde_json::to_string(&Action::Info {
                service: Some("0".into())
            })?
            .as_str()
        );
        assert_eq!(
            r#"{"method":"stop","params":{"service":"0"}}"#,
            serde_json::to_string(&Action::Stop {
                service: "0".into()
            })?
            .as_str()
        );
        Ok(())
    }

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
