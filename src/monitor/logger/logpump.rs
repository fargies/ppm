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

use anyhow::{Context, Result};
use std::{
    io::{ErrorKind, PipeReader, Read, Write, pipe, stdout},
    os::fd::{AsRawFd, RawFd},
    process::Stdio,
};

use crate::{
    monitor::logger::logfile::LogFile,
    utils::{Buffer, libc::NonBlock},
};

pub struct LogPump {
    pub input: Vec<PipeReader>,
    pub output: LogFile,
    buffer: Option<Buffer>,
}

impl From<LogFile> for LogPump {
    fn from(value: LogFile) -> Self {
        Self {
            input: Vec::with_capacity(2),
            output: value,
            buffer: None,
        }
    }
}

impl LogPump {
    pub fn on_input_ready(&mut self, fd: RawFd, mut buffer: Buffer) -> Option<Buffer> {
        let file = match self.input.iter_mut().find(|f| f.as_raw_fd() == fd) {
            Some(file) => file,
            None => {
                tracing::error!(fd, "unknown fd for logpump");
                return None;
            }
        };

        let ret = file.read(buffer.raw());
        match ret {
            Ok(sz) => {
                tracing::trace!(sz, fd, "bytes to log");
                match self.log(buffer.set_range(..sz).as_slice()) {
                    sz if !buffer.consume(sz).is_empty() => {
                        self.buffer = Some(buffer);
                        None
                    }
                    _ => Some(buffer),
                }
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => Some(buffer),
            Err(err) => {
                tracing::error!(?err, "input error");
                self.input.retain(|f| f.as_raw_fd() != fd);
                Some(buffer)
            }
        }
    }

    pub fn on_output_ready(&mut self, _fd: RawFd) -> Option<Buffer> {
        if let Some(mut buffer) = self.buffer.take() {
            match self.log(buffer.as_slice()) {
                n if !buffer.consume(n).is_empty() => {
                    self.buffer = Some(buffer);
                    None
                }
                _ => Some(buffer),
            }
        } else {
            None
        }
    }

    pub fn on_error(&mut self, fd: RawFd) -> Option<Buffer> {
        if let Some(index) = self.input.iter().position(|file| file.as_raw_fd() == fd) {
            tracing::error!(?fd, index, "error on input fd");
            self.input.remove(index);
            None
        } else if self.output.as_raw_fd().is_some_and(|out| out == fd) {
            tracing::error!(?fd, "error on output fd");
            self.buffer.take()
        } else {
            None
        }
    }

    pub fn on_hup(&mut self, fd: RawFd) -> Option<Buffer> {
        if let Some(index) = self.input.iter().position(|file| file.as_raw_fd() == fd) {
            /* hup is silent on inputs */
            tracing::trace!(?fd, "removing");
            self.input.remove(index);
            None
        } else {
            self.on_error(fd)
        }
    }

    ///send given buffer to logger
    ///
    ///Returns written bytes
    fn log(&mut self, buffer: &[u8]) -> usize {
        match self.output.write(buffer) {
            Ok(sz) => sz,
            Err(err) => {
                tracing::error!(?err, "failed to write log");
                // forwarding messages to stdout
                if let Err(err) = stdout().write_all(buffer) {
                    tracing::error!(?err, "failed to forward message");
                }
                buffer.len()
            }
        }
    }

    pub fn make_input(&mut self) -> Result<(Stdio, Stdio)> {
        let (reader_out, writer_out) = pipe().context("failed to create pipe")?;
        reader_out.set_nonblocking()?;
        let (reader_err, writer_err) = pipe().context("failed to create pipe")?;
        reader_err.set_nonblocking()?;

        tracing::trace!(
            fd_out = reader_out.as_raw_fd(),
            fd_err = reader_err.as_raw_fd(),
            "creating input pipe"
        );
        self.input.push(reader_out);
        self.input.push(reader_err);

        Ok((writer_out.into(), writer_err.into()))
    }

    pub fn has_buffer(&self) -> bool {
        self.buffer.is_some()
    }
}
