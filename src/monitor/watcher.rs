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

#[cfg(target_os = "macos")]
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
    use std::{
        fs::{File, create_dir_all},
        io::Write,
        path::{Path, PathBuf},
        sync::Arc,
        time::Duration,
    };

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

        let mon = Arc::new(Monitor {
            watch_restart_interval: Duration::from_millis(100),
            ..Default::default()
        });
        let service = {
            let mut srv = Service::new("test", Command::new("sleep", ["300"]));
            srv.watch = Some(yaml::from_str(
                format!("\"{}\"", temp.as_ref().to_str().unwrap()).as_str(),
            )?);
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
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(service.info().restarts, 2);

        Signal::kill(getpid(), signal::SIGTERM)?;
        join_handle.join().unwrap()?;
        Ok(())
    }

    #[test]
    #[serial(waitpid)]
    fn watch_file() -> Result<()> {
        let temp = MkTemp::dir("ppm-watch-file")?;
        let file = temp.as_ref().join("file");
        let watch_restart_interval = Duration::from_millis(100);

        let mon = Arc::new(Monitor {
            watch_restart_interval,
            ..Default::default()
        });
        File::create(&file)?;
        let service = {
            let mut srv = Service::new("test", Command::new("sleep", ["300"]));
            srv.watch = Some(yaml::from_str(
                format!("\"{}\"", file.to_str().unwrap()).as_str(),
            )?);
            mon.insert(srv)
        };
        let join_handle = {
            /* Monitor is handling dead processes */
            let mon = Arc::clone(&mon);
            std::thread::spawn(move || mon.run())
        };
        std::thread::sleep(Duration::from_millis(100));

        tracing::trace!(file = ?AsRef::<PathBuf>::as_ref(&temp), "deleting test file");
        File::options()
            .append(true)
            .open(&file)?
            .write_all(b"this is a test")?;
        std::thread::sleep(watch_restart_interval * 2);
        assert_eq!(service.info().restarts, 2);

        drop(temp);
        std::thread::sleep(watch_restart_interval * 2);
        assert_eq!(service.info().restarts, 3);

        Signal::kill(getpid(), signal::SIGTERM)?;
        join_handle.join().unwrap()?;
        Ok(())
    }

    fn make_path<S, I, K>(dir: S, iter: I) -> PathBuf
    where
        S: AsRef<PathBuf>,
        I: IntoIterator<Item = K>,
        K: AsRef<Path>,
    {
        iter.into_iter()
            .fold(dir.as_ref().clone(), |p, v| p.join(v.as_ref()))
    }

    #[test]
    #[serial(waitpid)]
    fn watch_filters() -> Result<()> {
        let temp = MkTemp::dir("ppm-watch-file")?;
        let file = MkTemp::file("ppm-watch-filter-file")?;
        let watch_restart_interval = Duration::from_millis(100);

        create_dir_all(make_path(&temp, ["valid", "subdir"]))?;
        create_dir_all(make_path(&temp, ["invalid", "subdir"]))?;

        let mon = Arc::new(Monitor {
            watch_restart_interval,
            ..Default::default()
        });
        let service = {
            let mut srv = Service::new("test", Command::new("sleep", ["300"]));
            srv.watch = Some(yaml::from_str(
                format!(
                    /* files with extension and paths are rejected by default, unless "valid" paths or .txt files */
                    "paths: [ '{}', '{}' ]\n\
                     include: [ '**/vali{{d,d/**}}', '*.txt' ]\n\
                     exclude : [ '*[.]*', '**[/]**' ]",
                    temp, file
                )
                .as_str(),
            )?);
            mon.insert(srv)
        };
        let join_handle = {
            /* Monitor is handling dead processes */
            let mon = Arc::clone(&mon);
            std::thread::spawn(move || mon.run())
        };
        std::thread::sleep(Duration::from_millis(100));

        /* items in `paths` are always watched */
        File::options()
            .append(true)
            .open(AsRef::<PathBuf>::as_ref(&file))?
            .write_all(b"test")?;
        std::thread::sleep(watch_restart_interval * 2);
        assert_eq!(service.info().restarts, 2);

        /* ignored files */
        File::create(make_path(&temp, ["invalid", "subdir", "toto.txt"]))?;
        File::create(make_path(&temp, ["valid", "subdir", "toto.not-txt"]))?;
        std::thread::sleep(watch_restart_interval * 2);
        assert_eq!(service.info().restarts, 2);

        File::create(make_path(&temp, ["valid", "subdir", "toto.txt"]))?;
        std::thread::sleep(watch_restart_interval * 2);
        assert_eq!(service.info().restarts, 3);

        /* directory is first processed as a file "another" and has to be validated */
        create_dir_all(make_path(&temp, ["valid", "subdir", "another"]))?;
        std::thread::sleep(watch_restart_interval * 2);
        assert_eq!(service.info().restarts, 4);

        /* watchs should have been re-created on paths, registering "another" */
        File::create(make_path(&temp, ["valid", "subdir", "another", "toto.txt"]))?;
        std::thread::sleep(watch_restart_interval * 2);
        assert_eq!(service.info().restarts, 5);

        Signal::kill(getpid(), signal::SIGTERM)?;
        join_handle.join().unwrap()?;
        Ok(())
    }
}
