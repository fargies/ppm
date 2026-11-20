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
** Created on: 2025-12-22T15:41:57
** Author: Sylvain Fargier <fargier.sylvain@gmail.com>
*/

use std::collections::HashMap;

use serde::{Deserialize, Serialize, ser::SerializeStruct};

#[derive(Debug, PartialEq, Default, Deserialize)]
#[serde(try_from = "CommandOptions")]
pub struct Command {
    pub path: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum CommandOptions {
    Plain(String),
    Array(Vec<String>),
    Object {
        path: String,
        #[serde(default)]
        args: Vec<String>,
        env: Option<HashMap<String, String>>,
    },
}

impl TryFrom<CommandOptions> for Command {
    type Error = anyhow::Error;

    fn try_from(value: CommandOptions) -> Result<Self, Self::Error> {
        match value {
            CommandOptions::Plain(value) => Command::parse(shlex::Shlex::new(value.as_str())),
            CommandOptions::Array(items) => Command::parse(items),
            CommandOptions::Object { path, args, env } => Ok(Command { path, args, env }),
        }
    }
}

impl Serialize for Command {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let is_env = !self.env.as_ref().is_none_or(|v| v.is_empty());
        let args_len = self.args.len();
        if !is_env && args_len == 0 {
            serializer.serialize_str(
                shlex::Quoter::new()
                    .quote(self.path.as_str())
                    .map_err(serde::ser::Error::custom)?
                    .as_ref(),
            )
        } else if !is_env && args_len < 4 {
            serializer.serialize_str(
                format!(
                    "{} {}",
                    shlex::Quoter::new()
                        .quote(self.path.as_str())
                        .map_err(serde::ser::Error::custom)?,
                    shlex::Quoter::new()
                        .join(self.args.iter().map(|v| v.as_str()))
                        .map_err(serde::ser::Error::custom)?
                )
                .as_str(),
            )
        } else {
            let mut out = serializer.serialize_struct(
                "Command",
                1 + if is_env { 1 } else { 0 } + if self.args.is_empty() { 0 } else { 1 },
            )?;
            out.serialize_field("path", &self.path)?;
            if args_len != 0 {
                out.serialize_field("args", &self.args)?;
            }
            if is_env {
                out.serialize_field("env", &self.env)?;
            }
            out.end()
        }
    }
}

impl Command {
    pub fn new<T, I, K>(path: T, args: I) -> Self
    where
        T: ToString,
        I: IntoIterator<Item = K>,
        K: ToString,
    {
        Self {
            path: path.to_string(),
            args: args.into_iter().map(|x| x.to_string()).collect(),
            env: None,
        }
    }

    pub fn parse<T>(value: T) -> anyhow::Result<Self>
    where
        T: IntoIterator<Item = String>,
    {
        let mut value = value.into_iter();
        let path = match value.next() {
            Some(path) => path,
            None => return Err(anyhow::anyhow!("empty command")),
        };
        Ok(Command {
            path,
            args: value.collect(),
            env: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use anyhow::{Context, Result};
    use serde_yaml_ng as yaml;

    use super::*;

    #[test]
    fn serde() -> Result<()> {
        let data = "ls -l -a\n";
        let cmd = Command::new("ls", ["-l", "-a"]);

        assert_eq!(data, yaml::to_string(&cmd).unwrap());
        assert_eq!(cmd, yaml::from_str::<Command>(data).unwrap());

        for (test, value) in [
            ("ls -la\n", Command::new("ls", ["-la"])),
            (
                ["path: ls", "args:", "- -l", "- -a", "- -l", "- -a", ""]
                    .join("\n")
                    .as_str(),
                Command::new("ls", ["-l", "-a", "-l", "-a"]),
            ),
            (
                ["path: ls", "env:", "  DEBUG: '*'", ""].join("\n").as_str(),
                Command {
                    path: "ls".to_string(),
                    args: Vec::new(),
                    env: Some(HashMap::from([(String::from("DEBUG"), String::from("*"))])),
                },
            ),
            (
                ["path: ls", "args:", "- -l", "env:", "  DEBUG: '*'", ""]
                    .join("\n")
                    .as_str(),
                Command {
                    path: "ls".to_string(),
                    args: vec![String::from("-l")],
                    env: Some(HashMap::from([(String::from("DEBUG"), String::from("*"))])),
                },
            ),
            ("ls\n", Command::new::<_, [&str; 0], _>("ls", [])),
        ] {
            assert_eq!(
                yaml::to_string(&value)?,
                test.to_string(),
                "failed to serialize: {value:?} -> {test}"
            );
            assert_eq!(
                value,
                serde_yaml_ng::from_str::<Command>(test)
                    .with_context(|| anyhow::anyhow!("failed to parse: {test}"))?,
                "failed to parse: {test} -> {value:?}"
            );
        }
        Ok(())
    }
}
