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

use crate::utils::{IntoArc, libc::NonBlock};
use anyhow::{Result, anyhow};
use chrono::SecondsFormat;
use regex::Regex;
use std::{
    fs::{self, File, remove_file},
    io::Write,
    os::fd::{AsRawFd, RawFd},
    path::PathBuf,
    sync::{Arc, LazyLock},
};

pub const LOGFILE_MAX_SIZE_DEFAULT: u64 = 1024 * 1024 * 20;
pub const LOGFILE_MAX_FILES_DEFAULT: usize = 3;
/* RFC3339 length + 1 : `-2345-78-01T34:67:90+23:56.log` */
const LOGFILE_SUFFIX_LEN: usize = 30;

static LOGFILE_SUFFIX_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"-\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\+\d{2}:\d{2}.log").unwrap());

pub struct LogFile {
    file: Option<File>,
    written: usize,
    pub max_size: u64,
    pub max_files: usize,
    log_dir: Arc<PathBuf>,
    log_name: String,
}

impl PartialEq<RawFd> for LogFile {
    fn eq(&self, other: &RawFd) -> bool {
        self.file.as_ref().is_some_and(|f| &f.as_raw_fd() == other)
    }
}

impl LogFile {
    #[allow(dead_code)] // used in tests
    pub fn new<T, S>(log_dir: T, log_name: S) -> Self
    where
        T: IntoArc<PathBuf>,
        S: Into<String>,
    {
        Self::new_with_limits(
            log_dir,
            log_name,
            LOGFILE_MAX_SIZE_DEFAULT,
            LOGFILE_MAX_FILES_DEFAULT,
        )
    }

    pub fn new_with_limits<T, S>(log_dir: T, log_name: S, max_size: u64, max_files: usize) -> Self
    where
        T: IntoArc<PathBuf>,
        S: Into<String>,
    {
        Self {
            file: None,
            written: 0,
            max_size,
            max_files,
            log_dir: log_dir.into_arc(),
            log_name: log_name.into(),
        }
    }

    pub fn as_raw_fd(&self) -> Option<RawFd> {
        self.file.as_ref().map(|f| f.as_raw_fd())
    }

    fn is_match(&self, filename: &str) -> bool {
        if filename.len() == self.log_name.len() + LOGFILE_SUFFIX_LEN {
            let (pre, suf) = filename.split_at(self.log_name.len());
            pre == self.log_name && LOGFILE_SUFFIX_RE.is_match(suf)
        } else {
            false
        }
    }

    pub fn make_filename(&self) -> String {
        format!(
            "{}-{}.log",
            self.log_name,
            chrono::Local::now().to_rfc3339_opts(SecondsFormat::Secs, false)
        )
    }

    pub fn list_files(&self) -> Vec<PathBuf> {
        match fs::read_dir(self.log_dir.as_path()) {
            Ok(rd) => {
                let mut ret = rd
                    .filter_map(|e| {
                        e.ok()
                            .filter(|e| {
                                e.file_name()
                                    .to_str()
                                    .is_some_and(|name| self.is_match(name))
                            })
                            .map(|e| e.path())
                    })
                    .collect::<Vec<PathBuf>>();
                ret.sort();
                ret
            }
            Err(err) => {
                tracing::error!(?err, "failed to open log dir");
                Vec::new()
            }
        }
    }

    pub fn rotate(&mut self) -> Result<()> {
        if self.file.is_some() && self.written < self.max_size as usize {
            return Ok(());
        }

        let files = self.list_files();
        let file = match files
            .last()
            .filter(|p| p.metadata().is_ok_and(|m| m.len() < self.max_size))
        {
            Some(file) => {
                tracing::info!(name = self.log_name, ?file, "existing log file found");
                File::options()
                    .append(true)
                    .open(file)
                    .inspect_err(|err| tracing::error!(?err, ?file, "failed to reopen log-file"))
                    .inspect(|f| {
                        self.written = f.metadata().map(|m| m.len()).unwrap_or(0) as usize;
                        if let Err(err) = f.set_nonblocking() {
                            tracing::error!(?err, "failed to set non-blocking");
                        }
                    })
            }
            None => {
                for file in files
                    .iter()
                    .take(files.len().saturating_sub(self.max_files - 1))
                {
                    tracing::debug!(name = self.log_name, ?file, "removing old log file");
                    if let Err(err) = remove_file(file) {
                        tracing::error!(?err, ?file, "failed to remove file");
                    }
                }

                let file = self.log_dir.join(self.make_filename());

                File::options()
                    .create(true)
                    .write(true)
                    .truncate(true)
                    .open(&file)
                    .inspect_err(|err| tracing::error!(?err, ?file, "failed to open log-file"))
                    .inspect(|f| {
                        tracing::info!(
                            fd = f.as_raw_fd(),
                            name = self.log_name,
                            ?file,
                            "{}",
                            if files.is_empty() {
                                "creating new log file"
                            } else {
                                "rotating log file"
                            }
                        );

                        self.written = 0;
                        f.set_nonblocking().unwrap_or_else(|err| {
                            tracing::error!(?err, "failed to set non-blocking")
                        })
                    })
            }
        };
        match file {
            Ok(file) => {
                self.file = Some(file);
                Ok(())
            }
            Err(err) => {
                self.file = None;
                Err(anyhow!(err))
            }
        }
    }

    pub fn write(&mut self, data: &[u8]) -> Result<usize> {
        self.rotate()
            .and_then(|()| {
                self.file
                    .as_mut()
                    .unwrap()
                    .write(data)
                    .map_err(anyhow::Error::new)
            })
            .inspect(|size| self.written += size)
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::MkTemp;
    use anyhow::Result;
    use std::time::Duration;

    use super::*;

    #[test]
    fn log_rotate() -> Result<()> {
        let temp_dir = MkTemp::dir("logger")?;
        let mut log = LogFile::new(temp_dir.clone(), "test");
        log.max_size = 10;
        log.max_files = 2;

        log.write(b"this is a test\n")?;
        assert_eq!(log.list_files().len(), 1);
        /* log files uses seconds granularity stamps */
        std::thread::sleep(Duration::from_secs(1));
        log.write(b"this is a test\n")?;
        assert_eq!(log.list_files().len(), 2);

        std::thread::sleep(Duration::from_secs(1));
        log.write(b"this is a test\n")?;
        assert_eq!(log.list_files().len(), 2);

        Ok(())
    }

    #[test]
    fn log_reopen() -> Result<()> {
        let temp_dir = MkTemp::dir("logger")?;
        let logfile = {
            let mut log = LogFile::new(temp_dir.clone(), "test");
            log.write(b"x\n")?;
            log.list_files()
                .first()
                .cloned()
                .expect("a file should have been created")
        };
        assert!(logfile.metadata().unwrap().len() <= 2);

        let logfile2 = {
            let mut log = LogFile::new(temp_dir.clone(), "test");
            log.write(b"x\n")?;
            log.list_files()
                .first()
                .cloned()
                .expect("a file should have been created")
        };
        assert_eq!(logfile, logfile2);
        assert!(logfile2.metadata().unwrap().len() > 2);

        Ok(())
    }
}
