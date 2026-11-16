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

use std::{
    io::{Error, ErrorKind, Result as IoResult},
    net::SocketAddr,
    os::fd::AsRawFd,
};

pub enum SocketType {
    Accepting,
    Connected,
}

pub trait SocketTrait {
    /// retrieve socket local address
    fn local_addr(&self) -> IoResult<SocketAddr>;

    /// retrieve connected socket remote address
    fn remote_addr(&self) -> IoResult<SocketAddr>;

    /// return true if socket is accepting new connections
    fn is_listening(&self) -> bool;

    /// return true if object is a socket
    fn is_socket(&self) -> bool;
}

fn make_sockaddr(addr: &libc::sockaddr_storage) -> IoResult<SocketAddr> {
    unsafe {
        match addr.ss_family as libc::c_int {
            libc::AF_INET => {
                let addr = &*(addr as *const _ as *const libc::sockaddr_in);

                Ok(SocketAddr::V4(std::net::SocketAddrV4::new(
                    std::net::Ipv4Addr::from(u32::from_be(addr.sin_addr.s_addr)),
                    u16::from_be(addr.sin_port),
                )))
            }
            libc::AF_INET6 => {
                let addr = &*(addr as *const _ as *const libc::sockaddr_in6);

                Ok(SocketAddr::V6(std::net::SocketAddrV6::new(
                    std::net::Ipv6Addr::from(addr.sin6_addr.s6_addr),
                    u16::from_be(addr.sin6_port),
                    addr.sin6_flowinfo,
                    addr.sin6_scope_id,
                )))
            }
            _ => Err(Error::new(
                ErrorKind::Unsupported,
                "Unsupported socket address family",
            )),
        }
    }
}

impl<K> SocketTrait for K
where
    K: AsRawFd,
{
    #[tracing::instrument(level = "ERROR", skip(self), err)]
    fn local_addr(&self) -> IoResult<SocketAddr> {
        let mut addr: libc::sockaddr_storage = unsafe { std::mem::zeroed() };
        let mut len = std::mem::size_of_val(&addr) as libc::socklen_t;

        let res =
            unsafe { libc::getsockname(self.as_raw_fd(), &mut addr as *mut _ as *mut _, &mut len) };
        super::check(res)?;

        make_sockaddr(&addr)
    }

    #[tracing::instrument(level = "ERROR", skip(self), err)]
    fn remote_addr(&self) -> IoResult<SocketAddr> {
        let mut addr: libc::sockaddr_storage = unsafe { std::mem::zeroed() };
        let mut len = std::mem::size_of_val(&addr) as libc::socklen_t;

        let res =
            unsafe { libc::getpeername(self.as_raw_fd(), &mut addr as *mut _ as *mut _, &mut len) };
        super::check(res)?;

        make_sockaddr(&addr)
    }

    fn is_listening(&self) -> bool {
        let mut accepting: libc::c_int = 0;
        let mut len = std::mem::size_of_val(&accepting) as libc::socklen_t;

        unsafe {
            libc::getsockopt(
                self.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_ACCEPTCONN,
                &mut accepting as *mut _ as *mut _,
                &mut len,
            ) == 0
                && accepting != 0
        }
    }

    fn is_socket(&self) -> bool {
        let mut socket_type: libc::c_int = 0;
        let mut len = std::mem::size_of_val(&socket_type) as libc::socklen_t;

        unsafe {
            libc::getsockopt(
                self.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_TYPE,
                &mut socket_type as *mut _ as *mut _,
                &mut len,
            ) == 0
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::MkTemp;

use super::*;
    use std::net::{TcpListener, TcpStream};

    #[test]
    fn socket_trait() -> IoResult<()> {
        let sock = TcpListener::bind("127.0.0.1:0")?;

        assert!(sock.is_socket());
        assert!(sock.is_listening());
        assert_eq!(
            sock.local_addr()?.to_string(),
            sock.as_raw_fd().local_addr()?.to_string()
        );
        sock.remote_addr().expect_err("should fail");

        let connected = TcpStream::connect(sock.local_addr()?)?;

        assert!(connected.is_socket());
        assert!(!connected.is_listening());
        assert_eq!(
            connected.local_addr()?.to_string(),
            connected.as_raw_fd().local_addr()?.to_string()
        );
        assert_eq!(
            sock.local_addr()?.to_string(),
            connected.as_raw_fd().remote_addr()?.to_string()
        );
        Ok(())
    }

    #[test]
    fn socket_trait_errors() -> IoResult<()> {
        let file = MkTemp::file("socket_trait")?;

        assert!(!file.is_socket());
        assert!(!file.is_listening());
        file.local_addr().expect_err("should fail");
        file.remote_addr().expect_err("should fail");
        Ok(())
    }
}
