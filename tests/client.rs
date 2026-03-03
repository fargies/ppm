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

use std::process::Command;

use anyhow::Result;
use serial_test::file_serial;

mod utils;
use utils::OnDrop;

const PPM_BIN: &str = std::env!("CARGO_BIN_EXE_ppm");
const SERVER_ADDR: &str = "127.0.0.1:34567";

fn ppm() -> Command {
    let mut ret = Command::new(PPM_BIN);
    ret.args(["--addr", SERVER_ADDR]);
    ret
}

#[test]
#[file_serial(server)]
fn cli_utils() -> Result<()> {
    let mut server = ppm().arg("daemon").spawn()?;
    let _server_guard = OnDrop::new(move || {
        unsafe {
            libc::kill(server.id() as i32, libc::SIGTERM);
        }
        assert!(
            server.wait().expect("failed to terminate server").success(),
            "server improper termination"
        );
        server.kill().expect("failed to kill server")
    });
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
