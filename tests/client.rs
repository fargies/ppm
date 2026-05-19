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
    io::Write,
    path::Path,
    process::{Child, Command},
};

use anyhow::Result;
use ppm::utils::{
    OnDrop,
    libc::{Fcntl, FdFlags},
};
use serial_test::file_serial;

mod utils;
use utils::{MkTemp, wait_for};

const PPM_BIN: &str = std::env!("CARGO_BIN_EXE_ppm");
const SERVER_ADDR: &str = "127.0.0.1:34567";

/* to eliminate this from coverage */
mod tests {
    use std::{io::Read, process::Stdio, time::Duration};

    use anyhow::Context;

    use super::*;

    fn ppm() -> Command {
        let mut ret = Command::new(PPM_BIN);
        ret.args(["--addr", SERVER_ADDR]);
        ret
    }

    fn kill_server(server: Child) {
        let mut server = server;
        unsafe {
            libc::kill(server.id() as i32, libc::SIGTERM);
        }
        assert!(
            server.wait().expect("failed to terminate server").success(),
            "server improper termination"
        );
        server.kill().expect("failed to kill server")
    }

    #[test]
    #[file_serial(server)]
    fn cli_utils() -> Result<()> {
        let server = ppm().arg("daemon").spawn()?;
        let _server_guard = OnDrop::new(|| kill_server(server));
        std::thread::sleep(std::time::Duration::from_secs(1));

        assert!(
            ppm()
                .args(["add", "--name", "test", "--", "sleep", "300"])
                .status()?
                .success(),
            "failed to add service"
        );
        assert!(
            ppm()
                .args(["add", "--name", "stopped", "--", "sleep", "300"])
                .status()?
                .success(),
            "failed to add service"
        );
        assert!(
            ppm().args(["stop", "stopped"]).status()?.success(),
            "failed to stop service"
        );

        assert!(
            ppm()
                .args(["add", "--name", "crashed", "--", "false"])
                .status()?
                .success(),
            "failed to add service"
        );

        assert!(
            !ppm().args(["unknown"]).status()?.success(),
            "unknown command should fail"
        );

        for cmd in [
            "info",
            "list",
            "ls",
            "status",
            "show-configuration",
            "show-config",
            "config",
            "stats",
            "statistics",
            "details",
            "show-scheduler",
        ] {
            assert!(
                ppm().arg(cmd).status()?.success(),
                "failed run command: {:?}",
                cmd
            );
        }

        for cmd in ["restart", "stop", "reschedule", "remove"] {
            assert!(
                ppm().arg(cmd).arg("test").status()?.success(),
                "failed to run command: {:?}",
                cmd
            );
        }

        Ok(())
    }

    #[test]
    #[file_serial(server)]
    fn cli_add_env() -> Result<()> {
        let server = ppm().arg("daemon").spawn()?;
        let _server_guard = OnDrop::new(|| kill_server(server));
        std::thread::sleep(std::time::Duration::from_secs(1));

        assert!(
            ppm()
                .args([
                    "add", "--name", "test", "--env", "TOTO=42", "--", "sleep", "300"
                ])
                .status()?
                .success(),
            "failed to add service"
        );
        assert!(
            !ppm()
                .args([
                    "add", "--name", "test", "--env", "TOTO", "--", "sleep", "300"
                ])
                .status()?
                .success(),
            "should fail to add service with invalid env"
        );

        Ok(())
    }

    #[test]
    #[file_serial(server)]
    fn cli_stats() -> Result<()> {
        let mut config = MkTemp::file("cli_stats")?;
        config.write_all(b"stats_interval: 250ms")?;

        let server = ppm()
            .arg("daemon")
            .arg(format!(
                "--config={}",
                AsRef::<Path>::as_ref(&config).display()
            ))
            .spawn()?;
        let _server_guard = OnDrop::new(|| kill_server(server));
        std::thread::sleep(std::time::Duration::from_secs(1));

        assert!(
            ppm()
                .args(["add", "--name", "test", "--", "sleep", "300"])
                .status()?
                .success(),
            "failed to add service"
        );
        std::thread::sleep(std::time::Duration::from_secs(1));

        assert!(
            ppm().arg("stats").status()?.success(),
            "failed to get stats"
        );

        Ok(())
    }

    #[test]
    #[file_serial(server)]
    fn cli_logs() -> Result<()> {
        let log_dir = MkTemp::dir("cli_logs")?;
        let mut config = MkTemp::file("cli_logs")?;
        config.write_all(format!("logger: {{ path: {:?} }}", log_dir.as_path()).as_bytes())?;

        let server = ppm()
            .arg("daemon")
            .arg(format!(
                "--config={}",
                AsRef::<Path>::as_ref(&config).display()
            ))
            .spawn()?;
        let _server_guard = OnDrop::new(|| kill_server(server));
        std::thread::sleep(std::time::Duration::from_secs(1));

        assert!(
            ppm()
                .args([
                    "add",
                    "--name",
                    "test",
                    "--env",
                    "RUST_LOG=",
                    "--",
                    "echo",
                    "world"
                ])
                .status()?
                .success(),
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

        wait_for!(
            str::from_utf8(&ppm().args(["log", "test"]).output()?.stdout)? == "world\n",
            "value: {:?}",
            ppm().args(["log", "test"]).output()?.stdout
        )?;

        assert!(
            ppm().args(["restart", "test"]).status()?.success(),
            "failed to restart service"
        );
        wait_for!(
            str::from_utf8(&ppm().args(["log", "test"]).output()?.stdout)? == "world\nworld\n",
            "value: {:?}",
            ppm().args(["log", "test"]).output()?.stdout
        )?;

        Ok(())
    }

    #[test]
    #[file_serial(server)]
    fn cli_daemon_logs() -> Result<()> {
        let log_dir = MkTemp::dir("cli_daemon_logs")?;
        let mut config = MkTemp::file("cli_daemon_logs")?;
        config.write_all(format!("logger: {{ path: {:?} }}", log_dir.as_path()).as_bytes())?;

        let server = ppm()
            .env("RUST_LOG", "info")
            .arg("daemon")
            .arg(format!(
                "--config={}",
                AsRef::<Path>::as_ref(&config).display()
            ))
            .spawn()?;
        let _server_guard = OnDrop::new(|| kill_server(server));
        std::thread::sleep(std::time::Duration::from_secs(1));

        assert!(
            ppm()
                .args(["add", "--name", "test", "--", "echo", "world"])
                .status()?
                .success(),
            "failed to add service"
        );

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

        wait_for!(
            !ppm()
                .args(["log", "ppm-daemon"])
                .output()?
                .stdout
                .is_empty()
        )?;
        Ok(())
    }

    #[test]
    #[file_serial(server)]
    fn cli_log_tracker() -> Result<()> {
        let log_dir = MkTemp::dir("cli_log_tracker")?;
        let mut config = MkTemp::file("cli_log_tracker")?;
        config.write_all(format!("logger: {{ path: {:?} }}", log_dir.as_path()).as_bytes())?;

        let server = ppm()
            .arg("daemon")
            .arg(format!(
                "--config={}",
                AsRef::<Path>::as_ref(&config).display()
            ))
            .spawn()?;
        let _server_guard = OnDrop::new(|| kill_server(server));
        std::thread::sleep(std::time::Duration::from_secs(1));

        assert!(
            ppm()
                .args([
                    "add",
                    "--name",
                    "test",
                    "--env",
                    "RUST_LOG=",
                    "--",
                    "echo",
                    "world"
                ])
                .status()?
                .success(),
            "failed to add service"
        );
        let mut tail = ppm()
            .args(["log", "-f", "test"])
            .stdout(Stdio::piped())
            .spawn()?;
        tail.stdout
            .as_mut()
            .expect("stdout not captured")
            .add_flag(FdFlags::NONBLOCK)?;
        let mut buf = [0; 10];
        wait_for!(
            {
                let n = tail
                    .stdout
                    .as_mut()
                    .expect("stdout not captured")
                    .read(&mut buf)?;
                str::from_utf8(&buf[..n])? == "world\n"
            },
            Duration::from_secs(3),
            "failed to get tail: {}",
            str::from_utf8(&buf)?
        )?;
        assert!(
            ppm().args(["restart", "test"]).status()?.success(),
            "failed to restart service"
        );

        buf.fill(0);
        wait_for!(
            {
                let n = tail
                    .stdout
                    .as_mut()
                    .expect("stdout not captured")
                    .read(&mut buf)?;
                str::from_utf8(&buf[..n])? == "world\n"
            },
            Duration::from_secs(3),
            "failed to get tail: {}",
            str::from_utf8(&buf)?
        )?;
        eprintln!("killing tail");
        unsafe { libc::kill(tail.id() as i32, libc::SIGTERM) };
        tail.wait()?;
        Ok(())
    }
}
