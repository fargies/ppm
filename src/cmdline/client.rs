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
** Created on: 2025-12-24T14:34:11
** Author: Sylvain Fargier <fargier.sylvain@gmail.com>
*/

use anyhow::{Context, Result};
use serde_json::Value;
use std::{
    io::{BufReader, Write},
    net::{TcpStream, ToSocketAddrs},
    time::Duration,
};

use super::Action;

#[derive(Debug)]
pub struct Client(TcpStream);

impl Client {
    pub fn connect<A: ToSocketAddrs>(addr: A) -> Result<Client> {
        let stream = TcpStream::connect(addr).context("failed to connect daemon")?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        Ok(Self(stream))
    }
}

impl Client {
    #[tracing::instrument(fields(client = ?self.0.local_addr()?, server = ?self.0.peer_addr()?, ?action), skip(self))]
    pub fn run(&self, action: &Action) -> Result<Value> {
        let mut reader = serde_json::Deserializer::from_reader(BufReader::new(&self.0))
            .into_iter::<serde_json::Value>();
        serde_json::to_writer(&self.0, &action)?;

        let reply = reader.next().unwrap().context("no reply from daemon")?;
        tracing::trace!(reply = ?reply, "reply");
        Ok(reply)
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use std::net::TcpListener;

    use super::*;

    #[test]
    fn client() -> Result<()> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let addr = listener.local_addr().unwrap();

        let cli = Client::connect(addr)?;
        cli.0.set_read_timeout(Some(Duration::from_secs(1)))?;

        assert!(
            cli.run(&Action::Info {})
                .unwrap_err()
                .to_string()
                .contains("no reply")
        );
        Ok(())
    }
}
