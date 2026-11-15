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

use anyhow::Result;
use clap::Parser;
use ppm::utils::{
    libc::{Dup, Fcntl, FdFlags},
    tracing_utils::tracing_init,
};
use std::{
    io::{ErrorKind, Write, stdin},
    net::{Shutdown, TcpListener},
    os::fd::{FromRawFd, IntoRawFd},
    time::Duration,
};

#[derive(Parser)]
struct Args {
    /// Text reply to send on the incoming socket
    #[arg(long, default_value_t = String::from("reply"))]
    pub reply: String,

    /// Loop on incoming sockets
    #[arg(long, default_value_t = false)]
    pub forever: bool,
}

///
/// Example TCP service
///
/// Used for unit-testing purpose (see [../src/monitor/port_listener.rs])
fn main() -> Result<()> {
    let args = Args::parse();
    tracing_init(std::io::stderr, Some("trace"))?;

    let receiver = unsafe { TcpListener::from_raw_fd(stdin().dup()?.into_raw_fd()) };
    receiver.add_flag(FdFlags::NONBLOCK)?;

    loop {
        match receiver.accept() {
            Ok((mut sock, addr)) => {
                println!("connection from {addr:?}, sending reply ...");
                sock.write_all(args.reply.as_bytes())?;
                sock.shutdown(Shutdown::Both)?;
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => {}
            Err(e) => {
                println!("error: {e:?}");
                return Err(e.into());
            }
        };
        if !args.forever {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    println!("tcp_service exit");
    Ok(())
}
