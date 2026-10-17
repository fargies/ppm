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

use std::{net::SocketAddr, sync::Arc};

use anyhow::{Context, Result, anyhow, Error};
use clap::Parser;
use ppm::api::ActionResult;
use ppm::utils::pre_json::JsonPreParser;
use ppm::{api::DEFAULT_SERVER_ADDR, utils::tracing_utils::tracing_init};
use salvo::hyper::body::Bytes;
use salvo::prelude::*;
use salvo::serve_static::static_embed;
use salvo::trailing_slash::remove_slash;
use salvo::websocket::{Message, WebSocketUpgrade};
use salvo::{logging::Logger, websocket::WebSocket};

mod web;
use tokio::io::AsyncWriteExt;
use web::{client::Client, pool::Pool};

#[derive(rust_embed::RustEmbed)]
#[folder = "src/web/app/dist"]
struct Assets;

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Args {
    #[arg(long, global = true, default_value_t = DEFAULT_SERVER_ADDR)]
    pub addr: SocketAddr,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_init(std::io::stderr, None)?;
    let args = Args::parse();

    let router = Router::new()
        .hoop(Logger::new())
        .hoop(affix_state::inject(Pool::<Client>::new(3)).insert("addr", args.addr))
        .push(Router::with_path("ws").goal(connect))
        .push(
            Router::with_path("{*path}")
                .hoop(remove_slash())
                .get(static_embed::<Assets>().fallback("index.html")),
        );

    let acceptor = TcpListener::new("0.0.0.0:3000").bind().await;
    Server::new(acceptor).serve(router).await;
    Ok(())
}

async fn ws_handle(
    ws: &mut WebSocket,
    pool: Arc<Pool<Client>>,
    addr: SocketAddr,
) -> anyhow::Result<()> {
    while let Some(msg) = ws.recv().await {
        let msg = msg.context("websocket error")?;
        if msg.is_close() {
            tracing::trace!("client closed");
            return Ok(());
        }

        let mut client = pool
            .acquire(&addr)
            .await
            .context("failed to acquire client")?;

        let reply = client
            .raw_invoke(msg.as_bytes())
            .await
            .context("failed to invoke method")?;
        ws.send(Message::binary(Bytes::copy_from_slice(reply)))
            .await?;
    }
    Ok(())
}

async fn ws_raw(
    ws: &mut WebSocket,
    pool: Arc<Pool<Client>>,
    addr: SocketAddr,
) -> anyhow::Result<()> {
    /* destructure the client */
    let Client {
        stream,
        read_buffer: JsonPreParser { mut buffer, .. },
    } = pool
        .acquire(&addr)
        .await
        .context("failed to acquire client")?
        .into_inner()
        .context("failed to extract client")?;
    let mut stream = match stream {
        Some(stream) => stream,
        None => return Err(anyhow!("client closed")),
    };

    loop {
        tokio::select! {
            msg = ws.recv() => {
                tracing::debug!("websocket msg received");
                let msg = match msg {
                    Some(msg) => msg.context("websocket error")?,
                    None => {
                        tracing::trace!("client closed");
                        return Ok(())
                    },
                };
                if msg.is_close() {
                    tracing::trace!("client close message");
                    return Ok(());
                }
                tracing::debug!("websocket msg sending");
                stream.write_all(msg.as_bytes()).await?;
                tracing::debug!("websocket msg sent");
            }
            ret = stream.readable() => {
                ret?;
                tracing::debug!("remote readable");
                buffer.clear();
                match
                stream.try_read_buf(&mut buffer) {
                    Ok(0) => return Err(Error::msg("remote connection closed")),
                    Ok(_) => (),
                    Err(ref e) if e.kind() == tokio::io::ErrorKind::WouldBlock => {
                        continue;
                    }
                    Err(e) => {
                        return Err(e.into());
                    }
                }
                tracing::warn!(log = ?String::from_utf8_lossy(buffer.as_slice()));
                ws.send(Message::binary(Bytes::copy_from_slice(buffer.as_slice()))).await?;
            }
        }
    }
    // Ok(())
}

#[handler]
async fn connect(req: &mut Request, res: &mut Response, depot: &Depot) -> Result<(), StatusError> {
    let pool: Arc<Pool<Client>> = depot
        .get_typed::<Arc<Pool<Client>>>()
        .map_err(|_| {
            tracing::error!("failed to acquire pool");
            StatusError::internal_server_error()
        })?
        .clone();
    let addr = *depot
        .get::<SocketAddr>("addr")
        .unwrap_or(&DEFAULT_SERVER_ADDR);
    let remote_addr = req.remote_addr().clone();
    let is_raw = req.query::<String>("raw").is_some();

    WebSocketUpgrade::new()
        .upgrade(req, res, async move |mut ws| {
            tracing::trace!(remote_addr=?remote_addr, is_raw, "websocket connected");
            let ret = if is_raw {
                ws_raw(&mut ws, pool, addr).await
            } else {
                ws_handle(&mut ws, pool, addr).await
            };
            if let Err(err) = ret {
                tracing::error!(?err, "websocket error");
                let _ = ws
                    .send(Message::binary(
                        serde_json::to_vec(&ActionResult::<()>::Err(format!(
                            "server error: {err:?}"
                        )))
                        .unwrap(),
                    ))
                    .await;
            };
            tracing::trace!(remote_addr=?remote_addr, "websocket closed");
        })
        .await
}
