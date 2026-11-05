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

use std::io::Result as IoResult;
use std::{
    mem::{size_of_val, zeroed},
    os::fd::{AsRawFd, RawFd},
    slice::{self, from_raw_parts},
};

use crate::utils::libc::check;

#[derive(Clone, Copy, Debug)]
pub enum ControlMessage<'a> {
    Rights(&'a [RawFd]),
    Unknown {
        cmsg_level: libc::c_int,
        cmsg_type: libc::c_int,
        payload: &'a [u8],
    },
}

impl<'a> From<&'a libc::cmsghdr> for ControlMessage<'a> {
    fn from(value: &'a libc::cmsghdr) -> Self {
        #[allow(clippy::unnecessary_cast)] /* type differs on macos/unix */
        let payload_size = value.cmsg_len as usize - size_of::<libc::cmsghdr>();
        match value {
            libc::cmsghdr {
                cmsg_level: libc::SOL_SOCKET,
                cmsg_type: libc::SCM_RIGHTS,
                ..
            } => Self::Rights(unsafe {
                from_raw_parts::<'a>(
                    libc::CMSG_DATA(value) as *const RawFd,
                    payload_size / size_of::<RawFd>(),
                )
            }),
            _ => Self::Unknown {
                cmsg_level: value.cmsg_level,
                cmsg_type: value.cmsg_type,
                payload: unsafe { from_raw_parts::<'a>(libc::CMSG_DATA(value), payload_size) },
            },
        }
    }
}

impl<'a> From<&'a [RawFd]> for ControlMessage<'a> {
    fn from(value: &'a [RawFd]) -> Self {
        ControlMessage::Rights(value)
    }
}

impl<'a> From<&'a RawFd> for ControlMessage<'a> {
    fn from(value: &'a RawFd) -> Self {
        ControlMessage::Rights(slice::from_ref(value))
    }
}

impl ControlMessage<'_> {
    pub fn data_size(&self) -> usize {
        match self {
            ControlMessage::Rights(items) => size_of_val(*items),
            ControlMessage::Unknown { payload, .. } => payload.len(),
        }
    }

    pub fn data_ptr(&self) -> *const u8 {
        match self {
            ControlMessage::Rights(items) => items.as_ptr().cast::<u8>(),
            ControlMessage::Unknown { payload, .. } => payload.as_ptr(),
        }
    }

    pub fn cmsg_level(&self) -> libc::c_int {
        match self {
            ControlMessage::Rights(..) => libc::SOL_SOCKET,
            ControlMessage::Unknown { cmsg_level, .. } => *cmsg_level,
        }
    }

    pub fn cmsg_type(&self) -> libc::c_int {
        match self {
            ControlMessage::Rights(..) => libc::SCM_RIGHTS,
            ControlMessage::Unknown { cmsg_type, .. } => *cmsg_type,
        }
    }
}

#[derive(Default, Clone)]
pub struct ControlRequest {
    data: Vec<u8>,
}

impl ControlRequest {
    pub fn new() -> Self {
        Self::with_capacity(1024)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
        }
    }

    pub fn clear(&mut self) {
        self.data.clear();
    }

    fn add_msg(&mut self, payload: *const u8, len: usize) -> &mut libc::cmsghdr {
        unsafe {
            let msg_len = libc::CMSG_SPACE(len as u32);
            let data_offset = self.data.len();
            self.data.resize(data_offset + msg_len as usize, 0);
            let cmsg = &mut *(self.data[data_offset..]
                .as_mut_ptr()
                .cast::<libc::cmsghdr>());
            cmsg.cmsg_len = libc::CMSG_LEN(len as u32) as libc::size_t;
            let data = libc::CMSG_DATA(cmsg);
            data.copy_from(payload, len);
            cmsg
        }
    }

    pub fn push(&mut self, msg: ControlMessage<'_>) {
        let cmsg = self.add_msg(msg.data_ptr(), msg.data_size());
        cmsg.cmsg_level = msg.cmsg_level();
        cmsg.cmsg_type = msg.cmsg_type();
    }

    pub fn iter<'a>(&'a self) -> ControlRequestIterator<'a> {
        ControlRequestIterator::from(self)
    }

    fn msghdr(&self) -> libc::msghdr {
        let mut msghdr: libc::msghdr = unsafe { zeroed() };
        msghdr.msg_control = self.data.as_ptr() as *mut libc::c_void;
        msghdr.msg_controllen = self.data.len() as libc::size_t;
        msghdr
    }
}

impl<'a, A> From<A> for ControlRequest
where
    A: Into<ControlMessage<'a>>,
{
    fn from(value: A) -> Self {
        let value = value.into();
        let mut ret =
            Self::with_capacity(unsafe { libc::CMSG_SPACE(value.data_size() as u32) as usize });
        ret.push(value);
        ret
    }
}

pub struct ControlRequestIterator<'a> {
    msg: libc::msghdr,
    cmsg: *const libc::cmsghdr,
    req: &'a ControlRequest,
}

impl<'a> From<&'a ControlRequest> for ControlRequestIterator<'a> {
    fn from(value: &'a ControlRequest) -> Self {
        let msg = value.msghdr();
        Self {
            msg,
            cmsg: unsafe { libc::CMSG_FIRSTHDR(&msg) },
            req: value,
        }
    }
}

impl<'a> Iterator for ControlRequestIterator<'a> {
    type Item = ControlMessage<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let ret = self.cmsg;
        if ret.is_null() {
            None
        } else {
            self.cmsg = unsafe { libc::CMSG_NXTHDR(&self.msg, ret) };
            Some(ControlMessage::from(unsafe { &*ret }))
        }
    }
}

pub trait SocketControlTrait {
    fn sendmsg(&self, buf: &[u8], req: &ControlRequest) -> IoResult<()>;
    fn recvmsg(&self, buf: &mut [u8], req: &mut ControlRequest) -> IoResult<usize>;
}

impl<K> SocketControlTrait for K
where
    K: AsRawFd,
{
    fn sendmsg(&self, buf: &[u8], req: &ControlRequest) -> IoResult<()> {
        let mut msghdr = req.msghdr();
        let mut iovec = libc::iovec {
            iov_base: buf.as_ptr() as *mut libc::c_void,
            iov_len: buf.len(),
        };
        msghdr.msg_iov = &mut iovec;
        msghdr.msg_iovlen = 1;

        let ret = unsafe { libc::sendmsg(self.as_raw_fd(), &msghdr, 0) };
        check(ret.min(0) as i32)
    }

    fn recvmsg(&self, buf: &mut [u8], req: &mut ControlRequest) -> IoResult<usize> {
        unsafe { req.data.set_len(req.data.capacity()) };
        let mut msghdr = req.msghdr();
        let mut iovec = libc::iovec {
            iov_base: buf.as_ptr() as *mut libc::c_void,
            iov_len: buf.len(),
        };
        msghdr.msg_iov = &mut iovec;
        msghdr.msg_iovlen = 1;
        let ret = unsafe { libc::recvmsg(self.as_raw_fd(), &mut msghdr, 0) };
        check(ret.min(0) as i32)?;
        unsafe {
            #[allow(clippy::unnecessary_cast)] /* type differs on macos/unix */
            req.data.set_len(msghdr.msg_controllen as usize);
        }
        Ok(ret as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::{
        MkTemp,
        libc::{Fcntl, FdFlags},
    };
    use anyhow::{Result, bail};
    use std::{
        fs::File,
        io::{Read, Seek, Write},
        os::{
            fd::{AsRawFd, FromRawFd},
            unix::net::UnixDatagram,
        },
    };

    use super::SocketControlTrait;

    #[test]
    fn sendmsg() -> Result<()> {
        let (sender, receiver) = UnixDatagram::pair()?;

        /* open temporary file, write something inside and rewind */
        let mut file = MkTemp::file("unix_datagram_sendmsg")?;
        file.write_all(b"pwetpwet")?;
        file.rewind()?;
        let fd = file.as_raw_fd();

        let mut msg = ControlRequest::from(&fd);
        sender.sendmsg(&[], &msg)?;

        msg.clear();
        let mut buf = vec![0u8; 10];
        assert_eq!(receiver.recvmsg(&mut buf, &mut msg)?, 0);

        assert_eq!(msg.iter().count(), 1);
        let new_fd = match msg.iter().next() {
            Some(ControlMessage::Rights(fds)) => {
                eprintln!("old fd: {fd}, new fds: {fds:?}");
                assert_eq!(fds.len(), 1);
                fds.first().copied().unwrap()
            }
            Some(ctrl) => bail!("invalid ctrl message: {ctrl:?}"),
            None => bail!("no ctrl messages"),
        };
        let mut new_file = unsafe { File::from_raw_fd(new_fd) };

        let mut value = String::default();
        new_file.read_to_string(&mut value)?;
        assert_eq!(value.as_str(), "pwetpwet");

        value.clear();
        file.read_to_string(&mut value)?;
        assert_eq!(value.as_str(), "");
        /* rewind the new file rewinds the old (same unerlying resource) */
        new_file.rewind()?;
        file.read_to_string(&mut value)?;
        assert_eq!(value.as_str(), "pwetpwet");

        drop(new_file);

        Ok(())
    }

    #[test]
    fn cloexec() -> Result<()> {
        let (sender, receiver) = UnixDatagram::pair()?;

        /* open temporary file, write something inside and rewind */
        let mut file = MkTemp::file("unix_datagram_cloexec")?;
        file.write_all(b"pwetpwet")?;
        file.rewind()?;
        let fd = file.as_raw_fd();
        assert!(fd.is_cloexec()?);
        file.add_flag(FdFlags::NONBLOCK)?;

        let mut msg = ControlRequest::from(&fd);
        sender.sendmsg(&[], &msg)?;

        msg.clear();
        assert_eq!(receiver.recvmsg(&mut [], &mut msg)?, 0);
        let new_fd = match msg.iter().next() {
            Some(ControlMessage::Rights(fds)) => {
                assert_eq!(fds.len(), 1);
                fds.first().copied().unwrap()
            }
            Some(ctrl) => bail!("invalid ctrl message: {ctrl:?}"),
            None => bail!("no ctrl messages"),
        };
        let new_file = unsafe { File::from_raw_fd(new_fd) };
        assert!(new_file.get_flags()?.contains(FdFlags::NONBLOCK));
        // WARNING: CLOEXEC is not passed on macos !!!
        #[cfg(not(target_os = "macos"))]
        assert!(new_file.is_cloexec()?);
        // there may be fd-leaks on macos if thread spawns a child process,
        // app must ensure this does not happen (global mutex or no child process)
        new_file.set_cloexec(true)?;

        Ok(())
    }
}
