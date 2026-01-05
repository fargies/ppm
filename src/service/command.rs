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

use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Command {
    pub path: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serde() {
        let data = "path: ls\n\
        args:\n\
        - -l\n\
        - -a\n\
        env: null\n";
        let cmd = Command::new("ls", ["-l", "-a"]);

        assert_eq!(data, serde_yaml_ng::to_string(&cmd).unwrap());
        assert_eq!(cmd, serde_yaml_ng::from_str::<Command>(data).unwrap());
    }
}
