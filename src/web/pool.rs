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

use std::{
    net::SocketAddr,
    ops::{Deref, DerefMut},
    sync::Arc,
};

use super::client::Client;
use anyhow::Result;
use tokio::sync::Mutex;

pub trait PoolItem: Send + Sync + 'static {
    fn is_alive(&self) -> bool;
}
impl PoolItem for Client {
    fn is_alive(&self) -> bool {
        self.stream.is_some()
    }
}

pub struct Pool<K: PoolItem> {
    // max_items: usize,
    keep_alive: usize,
    items: Mutex<Vec<K>>,
}

impl<K: PoolItem> Pool<K> {
    pub fn new(keep_alive: usize) -> Arc<Self> {
        Arc::new(Self {
            keep_alive,
            items: Default::default(),
        })
    }

    pub async fn release(&self, k: K) {
        let mut items = self.items.lock().await;
        if items.len() >= self.keep_alive {
            tracing::trace!("dropping item from pool");
            drop(items);
            drop(k);
        } else {
            tracing::trace!("releasing item to pool");
            items.push(k);
        }
    }
}

impl Pool<Client> {
    #[tracing::instrument(skip(self), err)]
    pub async fn acquire(self: &Arc<Self>, server_addr: &SocketAddr) -> Result<PoolRef<Client>> {
        if let Some(item) = self.items.lock().await.pop() {
            tracing::trace!(local_addr = ?item.local_addr(), "pool connection acquired");
            Ok(PoolRef {
                pool: Arc::clone(self),
                item: Some(item),
            })
        } else {
            let item = Client::connect(server_addr).await?;
            tracing::trace!(local_addr = ?item.local_addr(), "new connection created");
            Ok(PoolRef {
                pool: Arc::clone(self),
                item: Some(item),
            })
        }
    }
}

pub struct PoolRef<K: PoolItem> {
    pool: Arc<Pool<K>>,
    item: Option<K>,
}

impl<K: PoolItem> PoolRef<K> {
    pub fn into_inner(mut self) -> Option<K> {
        self.item.take()
    }
}

impl<K: PoolItem> Deref for PoolRef<K> {
    type Target = K;

    fn deref(&self) -> &Self::Target {
        self.item.as_ref().unwrap()
    }
}

impl<K: PoolItem> DerefMut for PoolRef<K> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.item.as_mut().unwrap()
    }
}

impl<K: PoolItem> Drop for PoolRef<K> {
    fn drop(&mut self) {
        if let Some(k) = self.item.take()
            && k.is_alive()
        {
            let pool = Arc::clone(&self.pool);
            tokio::spawn(async move { pool.release(k).await });
        }
    }
}
