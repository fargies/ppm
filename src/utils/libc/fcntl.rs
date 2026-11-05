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

use std::os::fd::AsRawFd;

use super::check;
use libc::{
    O_ACCMODE, O_DIRECTORY, O_DSYNC, O_EXCL, O_NONBLOCK, O_RDONLY, O_RDWR, O_SYNC, O_WRONLY, c_int,
};
use std::io::Result as IoResult;

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct FdFlags: c_int {
        const DIRECTORY = O_DIRECTORY; // Fail if file is a non-directory file.
        const EXCL = O_EXCL; // Exclusive use flag.
        const DSYNC = O_DSYNC; // Write according to synchronized I/O data integrity completion.
        const NONBLOCK = O_NONBLOCK; // Non-blocking mode.
        const SYNC = O_SYNC; // Write according to synchronized I/O file integrity completion.
        const ACCMODE = O_ACCMODE; // Mask for file access modes.
        const RDONLY = O_RDONLY; // Open for reading only.
        const RDWR = O_RDWR; // Open for reading and writing.
        const WRONLY = O_WRONLY; // Open for writing only.
    }
}

pub trait Fcntl {
    fn get_flags(&self) -> IoResult<FdFlags>;

    fn set_flags(&self, flags: FdFlags) -> IoResult<()>;

    #[inline]
    fn add_flag(&self, flag: FdFlags) -> IoResult<()> {
        let actual = self.get_flags()?;
        if !actual.contains(flag) {
            self.set_flags(actual | flag)?;
        }
        Ok(())
    }

    #[inline]
    fn remove_flag(&self, flag: FdFlags) -> IoResult<()> {
        let actual = self.get_flags()?;
        if actual.intersects(flag) {
            self.set_flags(actual.difference(flag))?;
        }
        Ok(())
    }

    fn is_cloexec(&self) -> IoResult<bool>;

    fn set_cloexec(&self, value: bool) -> IoResult<()>;
}

impl<T> Fcntl for T
where
    T: AsRawFd,
{
    fn get_flags(&self) -> IoResult<FdFlags> {
        unsafe {
            let flags = libc::fcntl(self.as_raw_fd(), libc::F_GETFL);
            check(flags.min(0)).and(Ok(FdFlags::from_bits_retain(flags)))
        }
    }

    fn set_flags(&self, flags: FdFlags) -> IoResult<()> {
        unsafe { check(libc::fcntl(self.as_raw_fd(), libc::F_SETFL, flags)) }
    }

    fn is_cloexec(&self) -> IoResult<bool> {
        unsafe {
            let flags = libc::fcntl(self.as_raw_fd(), libc::F_GETFD);
            check(flags.min(0)).and(Ok(flags & libc::FD_CLOEXEC != 0))
        }
    }

    fn set_cloexec(&self, value: bool) -> IoResult<()> {
        unsafe {
            let flags = libc::fcntl(self.as_raw_fd(), libc::F_GETFD);
            check(flags.min(0))?;
            if value {
                check(libc::fcntl(
                    self.as_raw_fd(),
                    libc::F_SETFD,
                    flags | libc::FD_CLOEXEC,
                ))
            } else {
                check(libc::fcntl(
                    self.as_raw_fd(),
                    libc::F_SETFD,
                    flags & !libc::FD_CLOEXEC,
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::MkTemp;
    use anyhow::Result;

    #[test]
    fn flags() -> Result<()> {
        let temp = MkTemp::file("fcntl")?;

        tracing::info!(flags = ?temp.as_raw_fd().get_flags()?);

        temp.add_flag(FdFlags::RDWR)?;
        tracing::info!(flags = ?temp.as_raw_fd().get_flags()?);
        assert!(temp.get_flags()?.contains(FdFlags::RDWR));

        temp.add_flag(FdFlags::NONBLOCK)?;
        tracing::info!(flags = ?temp.as_raw_fd().get_flags()?);
        assert!(temp.get_flags()?.contains(FdFlags::NONBLOCK));

        temp.remove_flag(FdFlags::NONBLOCK)?;
        tracing::info!(flags = ?temp.as_raw_fd().get_flags()?);
        assert!(!temp.get_flags()?.contains(FdFlags::NONBLOCK));

        Ok(())
    }

    #[test]
    fn cloexec() -> Result<()> {
        let temp = MkTemp::file("fcntl")?;

        assert!(temp.as_raw_fd().is_cloexec()?);
        Ok(())
    }
}
