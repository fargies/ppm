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

use anyhow::{Context, Result, anyhow};
use serde::de::DeserializeOwned;
use std::{
    collections::HashMap,
    io::BufReader,
    net::{TcpStream, ToSocketAddrs},
    time::Duration,
};
use tabled::{
    Table, Tabled,
    derive::display,
    settings::{
        Alignment, Color, Style,
        object::{Rows, Segment},
        style::BorderColor,
    },
};

use crate::{
    service::{self, Command, Service, ServiceId},
    utils,
};

use super::{Action, ActionResult};

#[derive(Debug)]
pub struct Client(TcpStream);

impl Client {
    #[tracing::instrument(level = "TRACE", skip(addr), ret, err)]
    pub fn connect<A: ToSocketAddrs>(addr: A) -> Result<Client> {
        let stream = TcpStream::connect(addr).context("failed to connect daemon")?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        Ok(Self(stream))
    }
}

impl Client {
    #[tracing::instrument(skip(self), ret(level = "TRACE"), err)]
    /// Invoke a single action
    pub fn invoke<R>(&self, action: &Action) -> Result<R>
    where
        R: DeserializeOwned + std::fmt::Debug,
    {
        let mut reader =
            serde_json::Deserializer::from_reader(BufReader::new(&self.0)).into_iter::<R>();
        serde_json::to_writer(&self.0, &action)?;
        tracing::trace!("action sent");

        let reply = reader.next().unwrap().context("no reply from daemon")?;
        Ok(reply)
    }

    #[tracing::instrument(fields(client = ?self.0.local_addr()?, server = ?self.0.peer_addr()?, ?action), skip(self))]
    /// Run a complete action, displaying result on the console
    pub fn run(&self, action: &Action) -> Result<()> {
        match action {
            Action::Daemon { .. } => unimplemented!("must be handled before connecting"),
            Action::List => unimplemented!("not available from cmdline"),
            Action::Info => {
                let services_list: HashMap<ServiceId, String> = self.invoke(&Action::List)?;
                let info: HashMap<ServiceId, service::Info> = self.invoke(&Action::Info)?;
                let mut keys: Vec<ServiceId> = info.keys().copied().collect();
                keys.sort();

                let data = keys
                    .iter()
                    .filter_map(|id| info.get(id).map(|x| (id, x)))
                    .map(|(id, info)| InfoRecord {
                        id: *id,
                        name: services_list.get(id),
                        info,
                    });
                self.display(Table::new(data));
                Ok(())
            }
            action @ (Action::Stop { .. } | Action::Restart { .. } | Action::Remove { .. }) => {
                self.0.set_read_timeout(Some(Duration::from_secs(30)))?;
                self.invoke::<ActionResult<()>>(&action)?.into()
            }
            Action::ShowConfiguration => match self.invoke::<ActionResult<String>>(&action)? {
                ActionResult::Ok(config) => {
                    print!("{config}");
                    Ok(())
                }
                action => action.map(|_| ()).into(),
            },
            action => self.invoke::<ActionResult<()>>(&action)?.into(),
        }
    }

    fn display(&self, mut table: Table) {
        table
            .with(Style::rounded().remove_horizontals())
            .with(Alignment::center());

        if utils::IS_OUT_COLORED.get() {
            table
                .modify(Rows::first(), Color::FG_BRIGHT_BLUE)
                .modify(Segment::all(), BorderColor::filled(Color::FG_BRIGHT_BLUE));
        }

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
