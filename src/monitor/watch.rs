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

use anyhow::Result;
use notify::{Watcher, recommended_watcher};
use std::{
    ops::{Deref, DerefMut},
    sync::mpsc,
};

/// Directory watching object
pub struct DirWatcher {
    watcher: Option<Box<dyn Watcher>>,
    join_handle: Option<std::thread::JoinHandle<()>>,
}

impl DirWatcher {
    pub fn new() -> Result<Self> {
        let (tx, rx) = mpsc::channel();

        let join_handle = std::thread::spawn(move || {
            tracing::trace!("running...");
            for event in rx {
                tracing::trace!(?event);
            }
            tracing::trace!("stopping...");
        });

        let watcher = Box::new(recommended_watcher(tx)?);
        tracing::trace!(watcher=?watcher, "watcher created");

        Ok(Self {
            watcher: Some(watcher),
            join_handle: Some(join_handle),
        })
    }
}

impl Deref for DirWatcher {
    type Target = Box<dyn Watcher>;

    fn deref(&self) -> &Self::Target {
        self.watcher.as_ref().unwrap()
    }
}

impl DerefMut for DirWatcher {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.watcher.as_mut().unwrap()
    }
}

impl Drop for DirWatcher {
    fn drop(&mut self) {
        self.watcher = None;
        if let Some(join_handle) = self.join_handle.take() {
            if let Err(err) = join_handle.join() {
                tracing::error!(?err, "failed to join DirWatcher thread");
            }
        }
        tracing::trace!("DirWatcher stopped");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;

    #[test]
    fn basic() -> Result<()> {
        let mut watcher = DirWatcher::new()?;
        Ok(())
    }
}
