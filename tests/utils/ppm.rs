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

use anyhow::{Context, Result};
use std::process::{Child, Command, Stdio};

pub const PPM_BIN: &str = std::env!("CARGO_BIN_EXE_ppm");
pub const SERVER_ADDR: &str = "127.0.0.1:34567";

pub const ECHO_SERVICE_ARGS: &[&str] = &[
    "add",
    "--name",
    "test",
    "--env",
    "RUST_LOG=",
    "--",
    "echo",
    "world",
];

pub fn ppm() -> Command {
    let mut ret = Command::new(PPM_BIN);
    ret.args(["--addr", SERVER_ADDR]);
    ret
}

pub fn kill_server(server: Child) {
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

pub fn wait_server() -> Result<()> {
    super::wait_for!(
        ppm()
            .args(["ls"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?
            .success()
    )
    .context("failed to connect server")
}
