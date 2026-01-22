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
use std::sync::Weak;

use crate::{
    monitor::Monitor,
    service::{ServiceId, Watch},
};

#[cfg(target_os = "linux")]
#[path = "watcher/inotify.rs"]
mod private;

#[cfg(any(target_os = "macos"))]
#[path = "watcher/fsevents.rs"]
mod private;

pub use private::Watcher;

pub trait WatcherTrait: Sized + Sync + Send {
    fn new(monitor: Weak<Monitor>) -> Result<Self>;

    fn add(&mut self, service_id: &ServiceId, watch: &Watch) -> Result<()>;

    fn remove(&mut self, service_id: &ServiceId);
}

#[cfg(test)]
mod tests {
    use std::{fs::File, sync::Arc, time::Duration};

    use crate::{
        service::{Command, Service},
        utils::{
            MkTemp,
            libc::getpid,
            signal::{self, Signal},
        },
    };
    use serde_yaml_ng as yaml;
    use serial_test::serial;

    use super::*;

    #[test]
    #[serial(waitpid)]
    fn watch() -> Result<()> {
        let temp = MkTemp::dir("ppm-watch")?;

        let mon = Arc::new(Monitor::default());
        let service = {
            let mut srv = Service::new("test", Command::new("sleep", ["300"]));
            srv.watch = Some(yaml::from_str(
                format!("\"{}\"", temp.as_ref().to_str().unwrap()).as_str(),
            )?);
            srv.start();
            mon.insert(srv)
        };

        let join_handle = {
            /* Monitor is handling dead processes */
            let mon = Arc::clone(&mon);
            std::thread::spawn(move || mon.run())
        };

        std::thread::sleep(Duration::from_millis(100));
        let file = temp.as_ref().join("test_file");
        tracing::trace!(?file, "creating test file");
        File::create(file)?;
        std::thread::sleep(Duration::from_millis(500));
        assert_eq!(service.info().restarts, 2);

        Signal::kill(getpid(), signal::SIGTERM)?;
        join_handle.join().unwrap()?;
        Ok(())
    }
}
