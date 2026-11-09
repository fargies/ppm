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

use ppm::utils::libc::{
    Dup, Fcntl, FdFlags,
    unix_datagram::{ControlMessage, ControlRequest, SocketControlTrait},
};
use std::{
    io::{Result, Write, stdin},
    net::{Shutdown, SocketAddr, TcpStream},
    os::{
        fd::{FromRawFd, IntoRawFd},
        unix::net::UnixDatagram,
    },
};

///
/// Example HTTP plugin
///
/// The `ppm-daemon` will create an `UnixDatagram` pipe and transfer incoming
/// HTTP requests TCPStreams through this pipe to the service identified as `http_service`.
fn main() -> Result<()> {
    let receiver = unsafe { UnixDatagram::from_raw_fd(stdin().dup()?.into_raw_fd()) };
    receiver.remove_flag(FdFlags::NONBLOCK)?;

    loop {
        let mut buf = [0u8; 255];
        let mut ctrl = ControlRequest::with_capacity(64);
        /* retrieve a control message */
        let len = receiver.recvmsg(&mut buf, &mut ctrl)?;

        let remote_addr: SocketAddr = serde_json::from_slice(&buf[..len])?;
        println!("connection request from {remote_addr:?}");

        for msg in ctrl.iter() {
            match msg {
                ControlMessage::Rights(items) => {
                    for el in items.iter() {
                        let mut stream = unsafe { TcpStream::from_raw_fd(*el) };
                        /* stub reply, for testing purpose */
                        stream.write_all(b"reply")?;
                        stream.shutdown(Shutdown::Both)?;
                    }
                }
                ControlMessage::Unknown {
                    cmsg_level,
                    cmsg_type,
                    ..
                } => eprintln!("unknown message: level={cmsg_level} type={cmsg_type}"),
            }
        }
    }
}
