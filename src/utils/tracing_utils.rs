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

#![allow(dead_code)]

use anyhow::Result;
use std::{env::var, io::IsTerminal, str::FromStr};

pub fn is_log_color<T>(output: &T) -> bool
where
    T: IsTerminal,
{
    match var("RUST_LOG_STYLE")
        .or_else(|_| var("LOG_COLOR"))
        .unwrap_or_else(|_| String::from("auto"))
        .to_lowercase()
        .as_str()
    {
        "never" | "no" | "0" | "false" => false,
        "always" | "yes" | "1" | "true" => true,
        _ => output.is_terminal(),
    }
}

fn get_var<S>(name: S) -> Option<bool>
where
    S: AsRef<str>,
{
    if let Ok(value) = var(name.as_ref()) {
        match value.to_lowercase().as_str() {
            "never" | "no" | "0" | "false" | "" => Some(false),
            /* "always" | "yes" | "1" | "true" */ _ => Some(true),
        }
    } else {
        None
    }
}

/// Initialize the tracing framework with sane defaults
///
/// ## Configuration from env
///
/// - LOG_SRC_FILE:    show source files (default `cfg!(test)`)
/// - LOG_THREAD_ID:   show thread ids (default `cfg!(test)`)
/// - LOG_THREAD_NAME: show thread names (default `false`)
/// - LOG_TARGET:      show log targets (default `false`)
/// - LOG_COLOR:       colorize logs (default `auto`)
/// - RUST_LOG | LOG_DIRECTIVE: log directive (default: `error`)
pub fn tracing_init<F, W>(output: F, directive: Option<&str>) -> Result<()>
where
    F: Fn() -> W + 'static + Send + Sync,
    W: std::io::Write + std::io::IsTerminal,
{
    use tracing::Level;
    use tracing_subscriber::{
        EnvFilter, Registry, filter::Directive, fmt, layer::SubscriberExt, util::SubscriberInitExt,
    };

    let log_src_file = get_var("LOG_SRC_FILE").unwrap_or(cfg!(test));
    let fmt = fmt::layer()
        .with_thread_ids(get_var("LOG_THREAD_ID").unwrap_or(cfg!(test)))
        .with_thread_names(get_var("LOG_THREAD_NAME").unwrap_or(false))
        .with_file(log_src_file)
        .with_line_number(log_src_file)
        .with_target(get_var("LOG_TARGET").unwrap_or(false))
        .with_ansi(is_log_color(&output()))
        .with_writer(output);

    #[cfg(test)]
    let fmt = fmt.with_test_writer();

    Registry::default()
        .with(
            EnvFilter::builder()
                .with_default_directive(
                    directive
                        .and_then(|v| Directive::from_str(v).ok())
                        .unwrap_or(Level::ERROR.into()),
                )
                .from_env_lossy(),
        )
        .with(fmt) // thread debugging
        .try_init()?;
    Ok(())
}
