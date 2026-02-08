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

#![allow(dead_code)]

use anyhow::{Context, Result};
use libc::{POLLERR, POLLHUP, POLLIN, POLLOUT, c_short, nfds_t, poll, pollfd};
use std::{
    io::{PipeReader, PipeWriter, Read, Write, pipe},
    os::fd::{AsRawFd, RawFd},
};

use crate::utils::libc::check;

/// Basic structure to help polling threads implementation
pub struct Poller {
    rx: PipeReader,
}

bitflags::bitflags! {
    /// Events to monitor
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct PollerFlags: c_short {
        const IN = POLLIN;
        const OUT = POLLOUT;
        const ERR = POLLERR;
        const HUP = POLLHUP;
    }
}

/// Set of file-descriptors to watch
pub struct PollerFds {
    pfds: Vec<pollfd>,
    events: Option<usize>,
}

impl PollerFds {
    /// Always add extra-room for the event pipe
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            pfds: Vec::with_capacity(capacity + 1),
            events: None,
        }
    }

    pub fn clear(&mut self) {
        self.events = None;
        self.pfds.clear();
    }

    pub fn push<T>(&mut self, fd: &T, flags: PollerFlags)
    where
        T: AsRawFd,
    {
        self.pfds.push(pollfd {
            fd: fd.as_raw_fd(),
            events: flags.bits(),
            revents: 0,
        })
    }

    pub fn iter(&self) -> impl Iterator<Item = (RawFd, PollerFlags)> {
        self.pfds
            .iter()
            .filter_map(|e| {
                if e.revents != 0 {
                    Some((e.fd, PollerFlags::from_bits_truncate(e.revents)))
                } else {
                    None
                }
            })
            .take(self.events.unwrap_or(self.pfds.len()))
    }
}

impl Poller {
    pub fn new() -> (Self, PollerWriter) {
        let (rx, tx) = pipe().expect("failed to create pipe");
        (Self { rx }, PollerWriter(tx))
    }

    pub fn poll(&mut self, pfds: &mut PollerFds) -> Result<Option<PollerWord>> {
        pfds.push(&self.rx, PollerFlags::IN);
        let ret = unsafe { poll(pfds.pfds.as_mut_ptr(), pfds.pfds.len() as nfds_t, -1) };
        check(ret.min(0)).context("failed to poll")?;
        if pfds.pfds.pop().is_some_and(|x| x.revents != 0) {
            pfds.events = Some((ret - 1) as usize);
            Ok(self.get_word())
        } else {
            pfds.events = Some(ret as usize);
            Ok(None)
        }
    }

    fn get_word(&mut self) -> Option<PollerWord> {
        let mut wake_word = [0u8; 1];
        if self.rx.read(&mut wake_word).unwrap_or(0) == 1 {
            Some(wake_word[0].into())
        } else {
            None
        }
    }
}

pub struct PollerWriter(PipeWriter);

impl PollerWriter {
    pub fn wake(&mut self) {
        if let Err(err) = self.0.write(&[PollerWord::Wake.discriminant()]) {
            tracing::error!(?err, "failed to send wake-word");
        }
    }

    pub fn exit(&mut self) {
        if let Err(err) = self.0.write(&[PollerWord::Exit.discriminant()]) {
            tracing::error!(?err, "failed to send exit-word");
        }
    }

    pub fn send(&mut self, value: u8) {
        if let Err(err) = self.0.write(&[value]) {
            tracing::error!(?err, value, "failed to send word");
        }
    }
}

#[derive(Debug)]
#[repr(u8)]
pub enum PollerWord {
    Wake = b'x',
    Exit = b'q',
    Custom(u8),
}

impl PollerWord {
    fn discriminant(&self) -> u8 {
        unsafe { *(self as *const Self as *const u8) }
    }
}

impl From<u8> for PollerWord {
    fn from(value: u8) -> Self {
        match value {
            b'x' => Self::Wake,
            b'q' => Self::Exit,
            n => Self::Custom(n),
        }
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use std::thread::JoinHandle;

    use crate::utils::debug::DebugIter;

    use super::*;

    /// Example poller object
    struct PollerExample {
        /* poller itself, will have internal wake/exit pipe */
        pub poller: PollerWriter,
        /* example data pipe usage */
        pub tx: PipeWriter,
        /* poller is running in a dedicated thread managed by this object */
        join_handle: Option<JoinHandle<Vec<u8>>>,
    }

    impl PollerExample {
        pub fn new() -> Self {
            let (mut poller, poller_writer) = Poller::new();
            let (mut rx, tx) = pipe().expect("failed to create pipe");

            let join_handle = std::thread::spawn(move || {
                let mut pfds = PollerFds::with_capacity(2);
                let mut out = Vec::with_capacity(4);

                loop {
                    pfds.clear();
                    pfds.push(&rx, PollerFlags::IN | PollerFlags::ERR);

                    let wake_word = poller.poll(&mut pfds).expect("failed to poll");
                    tracing::trace!(?wake_word, events = ?DebugIter::new(pfds.iter()), "poller event received");

                    for (fd, flags) in pfds.iter() {
                        tracing::trace!(fd, ?flags, "got poller event");
                        /* example usage, just saving data aside */
                        if flags.contains(PollerFlags::IN) && fd == rx.as_raw_fd() {
                            let mut buf = [0u8; 1];
                            if rx.read(&mut buf).unwrap_or(0) == 1 {
                                out.push(buf[0]);
                            }
                        }
                    }

                    if let Some(PollerWord::Exit) = wake_word {
                        break;
                    }
                }
                out
            });
            Self {
                poller: poller_writer,
                tx,
                join_handle: Some(join_handle),
            }
        }

        pub fn stop(&mut self) -> Option<Vec<u8>> {
            if let Some(handle) = self.join_handle.take() {
                self.poller.exit();
                Some(handle.join().expect("failed to join poller thread"))
            } else {
                None
            }
        }
    }

    impl Drop for PollerExample {
        fn drop(&mut self) {
            self.stop();
        }
    }

    #[test]
    fn poller() -> Result<()> {
        let mut ex = PollerExample::new();
        assert_eq!(4, ex.tx.write(&[1, 2, 3, 4])?);
        /* calling stop may drop some bytes otherwise */
        ex.poller.wake();
        ex.poller.send(b'u');
        ex.poller.send(b'v');
        assert_eq!(Some(vec![1, 2, 3, 4]), ex.stop());
        Ok(())
    }
}
