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
use anyhow::Result;
use libc::{
    O_ACCMODE, O_APPEND, O_CLOEXEC, O_CREAT, O_DIRECTORY, O_DSYNC, O_EXCL, O_NOCTTY, O_NOFOLLOW,
    O_NONBLOCK, O_RDONLY, O_RDWR, O_SYNC, O_TRUNC, O_WRONLY, c_int,
};

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct FdFlags: c_int {
        const CLOEXEC = O_CLOEXEC; // the file descriptor upon execution of an exec family function.
        const CREAT = O_CREAT; // Create file if it does not exist.
        const DIRECTORY = O_DIRECTORY; // Fail if file is a non-directory file.
        const EXCL = O_EXCL; // Exclusive use flag.
        const NOCTTY = O_NOCTTY; // Do not assign controlling terminal.
        const NOFOLLOW = O_NOFOLLOW; // Do not follow symbolic links.
        const TRUNC = O_TRUNC; // Truncate flag.
        const APPEND = O_APPEND; // Set append mode.
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
    fn get_flags(&self) -> Result<FdFlags>;

    fn set_flags(&self, flags: FdFlags) -> Result<()>;

    #[inline]
    fn add_flag(&self, flag: FdFlags) -> Result<()> {
        self.set_flags(self.get_flags()? | flag)
    }

    #[inline]
    fn remove_flag(&self, flag: FdFlags) -> Result<()> {
        self.set_flags(self.get_flags()?.difference(flag))
    }
}

impl<T> Fcntl for T
where
    T: AsRawFd,
{
    fn get_flags(&self) -> Result<FdFlags> {
        unsafe {
            let flags = libc::fcntl(self.as_raw_fd(), libc::F_GETFL);
            check(flags.min(0)).and(Ok(FdFlags::from_bits_retain(flags)))
        }
    }

    fn set_flags(&self, flags: FdFlags) -> Result<()> {
        unsafe { check(libc::fcntl(self.as_raw_fd(), libc::F_SETFL, flags)) }
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
}
