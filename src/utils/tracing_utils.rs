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
use std::{
    env::var,
    io::IsTerminal,
    str::FromStr,
    sync::{Arc, LazyLock, Mutex},
    time::Instant,
};
use tracing::Subscriber;
use tracing_subscriber::{
    fmt::{self, format},
    layer::SubscriberExt,
    registry::LookupSpan,
};

use crate::utils::slices::SliceIncludes;

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

pub fn make_fmt<S>()
-> fmt::Layer<S, format::DefaultFields, format::Format<format::Full, impl fmt::time::FormatTime>>
where
    S: Subscriber,
{
    let log_src_file = get_var("LOG_SRC_FILE").unwrap_or(cfg!(test));

    fmt::layer()
        .with_timer(fmt::time::ChronoLocal::rfc_3339())
        .with_thread_ids(get_var("LOG_THREAD_ID").unwrap_or(cfg!(test)))
        .with_thread_names(get_var("LOG_THREAD_NAME").unwrap_or(false))
        .with_file(log_src_file)
        .with_line_number(log_src_file)
        .with_target(get_var("LOG_TARGET").unwrap_or(false))
}

pub fn make_subscriber<F, W>(
    output: F,
    directive: Option<&str>,
) -> impl SubscriberExt + for<'a> LookupSpan<'a>
where
    F: Fn() -> W + 'static + Send + Sync,
    W: std::io::Write + std::io::IsTerminal,
{
    use tracing::Level;
    use tracing_subscriber::{EnvFilter, Registry, filter::Directive, layer::SubscriberExt};

    let fmt = make_fmt()
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
        .with(fmt)
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
    use tracing_subscriber::util::SubscriberInitExt;
    make_subscriber(output, directive).try_init()?;
    Ok(())
}

struct TracingDateTimeRef {
    pub next: Instant,
    pub stamp: Arc<String>,
}

impl Default for TracingDateTimeRef {
    fn default() -> Self {
        let now = Instant::now();
        let stamp = chrono::Local::now();
        let next = now
            + std::time::Duration::from_millis(
                1000u64.saturating_sub(stamp.timestamp_subsec_millis() as u64),
            );

        Self {
            next,
            stamp: Arc::new(stamp.to_rfc3339_opts(chrono::SecondsFormat::Secs, false) + " "),
        }
    }
}

impl TracingDateTimeRef {
    pub fn is_valid(&self) -> bool {
        self.next >= Instant::now()
    }
}

static TRACING_DATETIME_REF: LazyLock<Mutex<TracingDateTimeRef>> = LazyLock::new(Mutex::default);

pub struct TracingDateTime;

impl TracingDateTime {
    pub fn get() -> Arc<String> {
        let mut guard = TRACING_DATETIME_REF.lock().unwrap();
        if !guard.is_valid() {
            *guard = TracingDateTimeRef::default();
        }
        Arc::clone(&guard.stamp)
    }

    pub fn has_date_time(value: &[u8]) -> bool {
        // must contain year and hour: (format doesn't matter)
        let stamp = TracingDateTime::get();
        if stamp.len() >= 14 {
            value.includes_all([
                /* year */ stamp[..4].as_bytes(),
                /* hour: */ stamp[11..14].as_bytes(),
            ])
        } else {
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::Datelike;

    use super::*;

    #[test]
    fn tracing_datetime() {
        let date = TracingDateTime::get();
        assert!(!date.is_empty());
        std::thread::sleep(std::time::Duration::from_secs(1));

        assert_ne!(date, TracingDateTime::get());
    }

    #[test]
    fn tracing_has_datetime() {
        let date = chrono::Local::now();
        assert!(TracingDateTime::has_date_time(
            format!("{} {}", date.year(), date.time()).as_bytes()
        ));

        assert!(!TracingDateTime::has_date_time(b"hello world"));
        assert!(TracingDateTime::has_date_time(
            TracingDateTime::get().as_bytes()
        ));
    }
}
