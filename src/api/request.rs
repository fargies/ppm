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

use std::borrow::Cow;

use super::{Action, ActionResult};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Eq, PartialEq, Clone)]
#[serde(untagged)]
pub enum RequestId {
    String(String),
    Int(i64),
}

impl From<i64> for RequestId {
    fn from(value: i64) -> Self {
        RequestId::Int(value)
    }
}

impl From<String> for RequestId {
    fn from(value: String) -> Self {
        RequestId::String(value)
    }
}

impl std::fmt::Debug for RequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::String(value) => write!(f, "{value:?}"),
            Self::Int(value) => write!(f, "{value}"),
        }
    }
}

#[derive(Serialize, Deserialize, Eq, PartialEq, Debug)]
pub struct Request {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<RequestId>,
    #[serde(flatten)]
    pub action: Action,
}

impl Request {
    pub fn new<T>(id: T, action: Action) -> Self
    where
        T: Into<RequestId>,
    {
        Self {
            id: Some(id.into()),
            action,
        }
    }
}

impl From<Action> for Request {
    fn from(action: Action) -> Self {
        Self { id: None, action }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Reply<'a, T> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Cow<'a, Option<RequestId>>,
    #[serde(flatten)]
    pub result: ActionResult<T>,
}

impl<'a, T> Reply<'a, T>
where
    T: Serialize,
{
    #[tracing::instrument(skip(value))]
    pub fn ok(id: &'a Option<RequestId>, value: T) -> Self {
        Self {
            id: Cow::Borrowed(id),
            result: ActionResult::Ok(value),
        }
    }
}

impl<'a, T> Reply<'a, T> {
    pub fn err<E>(id: &'a Option<RequestId>, error: E) -> Self
    where
        E: ToString,
    {
        Self {
            id: Cow::Borrowed(id),
            result: ActionResult::Err(error.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;

    #[test]
    fn request_serde() -> Result<()> {
        let values: &[(&str, Request)] = &[
            (
                r#"{"id": 12, "method": "list"}"#,
                Request::new(12, Action::List),
            ),
            (r#"{"method": "list"}"#, Request::from(Action::List)),
            (
                r#"{"method": "info","params":{}}"#,
                Request::from(Action::Info { service: None }),
            ),
            (
                r#"{"method": "info","params":{"service":"test"}}"#,
                Request::from(Action::Info {
                    service: Some("test".into()),
                }),
            ),
            (
                r#"{"jsonrpc":"2.0","method":"stop", "params":{"service":"bar"}}"#,
                Request::from(Action::Stop {
                    service: "bar".into(),
                }),
            ),
        ];
        for (str, request) in values {
            let result: Request = serde_json::from_str(str)?;
            assert_eq!(request, &result);
        }
        Ok(())
    }
}
