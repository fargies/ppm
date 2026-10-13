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

use std::{net::SocketAddr, ops::DerefMut};

use anyhow::{Error, Result};
use ppm::{
    api::{Action, ActionResult},
    utils::pre_json::JsonPreParser,
};
use serde::de::DeserializeOwned;
use tokio::{
    io::AsyncWriteExt,
    net::{TcpStream, ToSocketAddrs},
};

pub struct Client {
    pub stream: Option<TcpStream>,
    read_buffer: JsonPreParser,
    write_buffer: Vec<u8>,
}

impl Client {
    pub async fn connect<A>(addr: A) -> Result<Self>
    where
        A: ToSocketAddrs,
    {
        Ok(Self {
            stream: Some(TcpStream::connect(addr).await?),
            read_buffer: Default::default(),
            write_buffer: Vec::with_capacity(4096),
        })
    }

    pub fn local_addr(&self) -> Option<SocketAddr> {
        self.stream.as_ref().and_then(|s| s.local_addr().ok())
    }

    #[tracing::instrument(skip(self, data), err)]
    pub async fn raw_invoke<'a>(&'a mut self, data: &[u8]) -> Result<&'a [u8]> {
        let mut stream = match self.stream.take() {
            Some(stream) => stream,
            None => return Err(Error::msg("client closed")),
        };

        stream.write_all(data).await?;
        tracing::trace!(size=data.len(), "raw action sent");
        loop {
            tokio::time::timeout(std::time::Duration::from_secs(5), stream.readable()).await??;

            match stream.try_read_buf(self.read_buffer.deref_mut()) {
                Ok(0) => return Err(Error::msg("connection closed")),
                Ok(_) => (),
                Err(ref e) if e.kind() == tokio::io::ErrorKind::WouldBlock => {
                    continue;
                }
                Err(e) => {
                    return Err(e.into());
                }
            };
            if self.read_buffer.parse() {
                /* restore the stream */
                self.stream = Some(stream);
                let data = self.read_buffer.get().unwrap();
                tracing::trace!(size=data.len(), "action reply");
                return Ok(data);
            }
        }
    }

    #[tracing::instrument(skip(self), ret(level = "TRACE"), err)]
    pub async fn invoke<R>(&mut self, action: &Action) -> Result<ActionResult<R>>
    where
        R: DeserializeOwned + std::fmt::Debug,
    {
        self.write_buffer.clear();
        serde_json::to_writer(&mut self.write_buffer, &action)?;
        let mut stream = match self.stream.take() {
            Some(stream) => stream,
            None => return Err(Error::msg("client closed")),
        };
        stream.write_all(self.write_buffer.as_ref()).await?;

        tracing::trace!("action sent");
        loop {
            tokio::time::timeout(std::time::Duration::from_secs(5), stream.readable()).await??;

            match stream.try_read_buf(self.read_buffer.deref_mut()) {
                Ok(0) => return Err(Error::msg("connection closed")),
                Ok(_) => (),
                Err(ref e) if e.kind() == tokio::io::ErrorKind::WouldBlock => {
                    continue;
                }
                Err(e) => {
                    return Err(e.into());
                }
            }
            if self.read_buffer.parse() {
                let data = self.read_buffer.get().unwrap();
                let reply: ActionResult<R> = serde_json::from_slice(data)?;

                /* restore the stream */
                self.stream = Some(stream);
                return Ok(reply);
            }
        }
    }
}
