/*
** Copyright (C) 2026 Sylvain Fargier
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

//! Invoke methods that are not reachable from command-line cli

mod utils;

/* to eliminate this from coverage */
mod tests {
    use std::{
        io::{self, Read, Write},
        net::Shutdown,
        path::Path,
    };

    use anyhow::{Context, Result};
    use ppm::{
        api::Request,
        cmdline::Client,
        utils::{OnDrop, tracing_utils::tracing_init},
    };
    use serial_test::file_serial;

    use crate::utils::{
        MkTemp,
        ppm::{ECHO_SERVICE_ARGS, SERVER_ADDR, kill_server, ppm, wait_server},
        wait_for,
    };

    #[ctor::ctor(unsafe)]
    fn prepare() {
        tracing_init(std::io::stdout, Some("debug")).expect("failed to init tests-tracing");
    }

    #[test]
    #[file_serial(server)]
    fn server_forwarded_logs() -> Result<()> {
        let log_dir = MkTemp::dir("cli_logs")?;
        let mut config = MkTemp::file("cli_logs")?;
        config.write_all(
            format!(
                "logger: {{ path: {:?}, auto_date: false }}",
                log_dir.as_path()
            )
            .as_bytes(),
        )?;

        let server = ppm()
            .arg("daemon")
            .arg(format!(
                "--config={}",
                AsRef::<Path>::as_ref(&config).display()
            ))
            .spawn()?;
        let _server_guard = OnDrop::new(|| kill_server(server));
        wait_server()?;

        assert!(
            ppm().args(ECHO_SERVICE_ARGS).status()?.success(),
            "failed to add service"
        );

        wait_for!(
            log_dir
                .read_dir()?
                .filter(|f| f
                    .as_ref()
                    .is_ok_and(|f| f.file_name().to_string_lossy().starts_with("test-")))
                .count()
                == 1
        )
        .context("log file not created")?;

        let mut conn = Client::connect(SERVER_ADDR)?;
        conn.set_nonblocking(true)?;
        conn.write_all(
            serde_json::to_vec(&Request::new(
                "test".to_string(),
                ppm::api::Action::Log {
                    service: "test".to_string(),
                    lines: Some(10),
                    follow: Some(true),
                },
            ))?
            .as_slice(),
        )?;

        let mut buf = String::with_capacity(128);
        wait_for!(
            conn.read_to_string(&mut buf)
                .or_else(|err| if err.kind() == io::ErrorKind::WouldBlock {
                    Ok(0)
                } else {
                    Err(err)
                })
                .is_ok_and(|_| buf.contains("world")),
            "failed to get log: {}",
            buf
        )?;
        conn.shutdown(Shutdown::Both)?;

        Ok(())
    }

    #[test]
    #[file_serial(server)]
    fn clock_check() -> Result<()> {
        let log_dir = MkTemp::dir("cli_logs")?;
        let mut config = MkTemp::file("cli_logs")?;
        config.write_all(
            format!(
                "logger: {{ path: {:?}, auto_date: false }}",
                log_dir.as_path()
            )
            .as_bytes(),
        )?;

        let server = ppm()
            .arg("daemon")
            .arg(format!(
                "--config={}",
                AsRef::<Path>::as_ref(&config).display()
            ))
            .spawn()?;
        let _server_guard = OnDrop::new(|| kill_server(server));
        wait_server()?;

        wait_for!(
            log_dir
                .read_dir()?
                .filter(|f| f
                    .as_ref()
                    .is_ok_and(|f| f.file_name().to_string_lossy().starts_with("ppm-daemon-")))
                .count()
                == 1
        )
        .context("log file not created")?;

        let conn = Client::connect(SERVER_ADDR)?;
        conn.invoke::<()>(&ppm::api::Action::ClockCheck)?;

        assert!(
            ppm().args(ECHO_SERVICE_ARGS).status()?.success(),
            "failed to add service"
        );
        /* this ensures clock-check was actually executed by waiting on service creation */
        wait_for!(
            log_dir
                .read_dir()?
                .filter(|f| f
                    .as_ref()
                    .is_ok_and(|f| f.file_name().to_string_lossy().starts_with("test-")))
                .count()
                == 1
        )
        .context("log file not created")?;

        Ok(())
    }
}
