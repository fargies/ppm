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
use serde::de::DeserializeOwned;
use std::{
    collections::HashMap,
    io::BufReader,
    net::{TcpStream, ToSocketAddrs},
    time::Duration,
};
use tabled::{
    derive::display, grid::config::Borders, settings::{object::{Columns, Rows, Segment}, style::BorderColor, themes::Colorization, Alignment, Color, Rotate, Style, Theme, Width}, Table, Tabled
};

use crate::service::{self, ServiceId};

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
    #[tracing::instrument(skip(self), ret(level = "TRACE"))]
    /// Invoke a single action
    pub fn invoke<R>(&self, action: &Action) -> Result<R>
    where
        R: DeserializeOwned + std::fmt::Debug,
    {
        let mut reader =
            serde_json::Deserializer::from_reader(BufReader::new(&self.0)).into_iter::<R>();
        serde_json::to_writer(&self.0, &action)?;

        let reply = reader.next().unwrap().context("no reply from daemon")?;
        Ok(reply)
    }

    #[tracing::instrument(fields(client = ?self.0.local_addr()?, server = ?self.0.peer_addr()?, ?action), skip(self))]
    /// Run a complete action, displaying result on the console
    pub fn run(&self, action: &Action) -> Result<()> {
        match action {
            Action::Daemon => unimplemented!("must be handled before connecting"),
            Action::List => unimplemented!("not available from cmdline"),
            Action::Info => {
                let services_list: HashMap<ServiceId, String> = self.invoke(&Action::List)?;
                let info: HashMap<ServiceId, service::Info> = self.invoke(&Action::Info)?;
                let data = info.iter().map(|(id, info)| InfoRecord {
                    id: *id,
                    name: services_list.get(id),
                    info: &info,
                });
                let mut table = Table::new(data);
                self.display(table);
            }
        }
        Ok(())
    }

    fn display(&self, mut table: Table) {
        table
        .with(Style::rounded().remove_horizontals())
        // .modify(Rows::first(), Color::FG_CYAN)
        // .modify(Segment::all(), BorderColor::filled(Color::FG_CYAN))
        .with(Alignment::center());

        println!("{}", table);
    }
}

#[derive(Tabled)]
struct InfoRecord<'a, 'b> {
    id: ServiceId,
    #[tabled(display("display::option", ""))]
    name: Option<&'a String>,
    #[tabled(inline)]
    info: &'b service::Info,
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
