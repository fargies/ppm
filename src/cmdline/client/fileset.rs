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
    fs::File,
    io::{self, ErrorKind, Read, Seek, SeekFrom},
    os::unix::fs::MetadataExt,
    path::PathBuf,
};

#[derive(Debug)]
struct FileInfo {
    start: u64,
    size: u64,
}

type FileIndex = usize;

#[derive(Debug)]
pub struct FileSet {
    files: Vec<PathBuf>,
    sizes: Vec<FileInfo>,
    total_size: u64,
    /// Currently opened file index
    index: FileIndex,
    /// Relative file position
    rpos: u64,
    current: File,
}

/// Consider a set of files as a single file
///
/// implements [Seek] and [Read]
impl FileSet {
    pub fn new(files: Vec<PathBuf>) -> io::Result<Self> {
        if files.is_empty() {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                "no files in the set",
            ));
        }
        let mut sizes = Vec::with_capacity(files.len());
        let mut start = 0;
        for path in files.iter() {
            let size = path.metadata()?.size();
            sizes.push(FileInfo { start, size });
            start += size;
        }
        Ok(Self {
            sizes,
            index: 0,
            total_size: start,
            current: File::open(files.first().unwrap())?,
            rpos: 0,
            files,
        })
    }

    fn open_idx(&mut self, index: FileIndex) -> io::Result<()> {
        if self.index == index {
            return Ok(());
        }
        match self.files.get(index) {
            Some(path) => {
                self.current = File::open(path)?;
                self.index = index;
                self.rpos = 0;
                Ok(())
            }
            None => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("no such index for file {index}").as_str(),
            )),
        }
    }

    /// open at global offset
    ///
    /// returns global position in file-set, may be different from requested
    /// pos if outside of boundaries
    fn open_at(&mut self, pos: u64) -> io::Result<u64> {
        let mut file_relative_pos = pos;
        match self.sizes.iter().position(|s| {
            if s.size < file_relative_pos {
                file_relative_pos -= s.size;
                false
            } else {
                true
            }
        }) {
            Some(idx) => {
                self.open_idx(idx)?;
                self.rpos = self.current.seek(SeekFrom::Start(file_relative_pos))?;
                Ok(self.sizes[self.index].start + self.rpos)
            }
            None => {
                self.open_idx(self.files.len() - 1)?;
                self.rpos = self.current.seek(SeekFrom::End(0))?;
                Ok(self.total_size)
            }
        }
    }

    #[inline]
    fn get_position(&self) -> u64 {
        self.rpos + self.sizes[self.index].start
    }

    #[inline]
    fn get_file_remaining(&self) -> u64 {
        self.sizes[self.index].size.saturating_sub(self.rpos)
    }
}

impl Read for FileSet {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let buf_sz = buf.len().min(self.get_file_remaining() as usize);
        match self.current.read(&mut buf[..buf_sz]) {
            Ok(size) if size == buf.len() => {
                self.rpos += size as u64;
                Ok(size)
            }
            Ok(size) => {
                if self.index < self.files.len() - 1 {
                    self.open_idx(self.index + 1)?;
                    self.read(&mut buf[size..]).map(|s| s + size)
                } else {
                    self.rpos += size as u64;
                    Ok(size)
                }
            }
            err @ Err(_) => err,
        }
    }
}

impl Seek for FileSet {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        match pos {
            SeekFrom::Start(pos) => self.open_at(pos),
            SeekFrom::End(pos) => self.open_at(self.total_size.saturating_add_signed(pos)),
            SeekFrom::Current(pos) => self.open_at(self.get_position().saturating_add_signed(pos)),
        }
    }
}

/// Convert the [FileSet] into the current file being read
impl From<FileSet> for File {
    fn from(value: FileSet) -> Self {
        value.current
    }
}

#[cfg(test)]
mod tests {
    use std::{ffi::OsStr, io::Write};

    use super::*;
    use crate::utils::MkTemp;
    use anyhow::Result;

    #[test]
    fn file_set() -> Result<()> {
        let dir = MkTemp::dir("file_set")?;
        let mut fileset = Vec::with_capacity(10);

        /* prepare 10 files of 10x10 chars */
        for i in 0..10 {
            let filename = dir.as_path().join(format!("file{i}").as_str());

            let mut file = File::create(&filename)?;
            for _ in 0..10 {
                writeln!(
                    &mut file,
                    "{}0123",
                    filename.file_name().and_then(OsStr::to_str).unwrap()
                )?;
            }
            fileset.push(filename);
        }

        let mut fileset = FileSet::new(fileset)?;
        assert_eq!(10 * 10 * 10, fileset.seek(SeekFrom::End(0))?);

        let mut buf = [0u8; 10];
        assert_eq!(10 * 10 - 5, fileset.seek(SeekFrom::Start(10 * 10 - 5))?);
        assert_eq!(10, fileset.read(&mut buf)?);
        assert_eq!(b"0123\nfile1", &buf);

        assert_eq!(10 * 9, fileset.seek(SeekFrom::Current(-15))?);
        assert_eq!(10, fileset.read(&mut buf)?);
        assert_eq!(b"file00123\n", &buf);
        assert_eq!(10, fileset.read(&mut buf)?);
        assert_eq!(b"file10123\n", &buf);
        assert_eq!(10 * 11, fileset.stream_position()?);
        assert_eq!(10 * 19, fileset.seek(SeekFrom::Current(10 * 8))?);

        let mut file: File = fileset.into();
        let mut buf = Vec::with_capacity(10);
        file.read_to_end(&mut buf)?;
        assert_eq!(buf, b"file10123\n");
        Ok(())
    }

    /// Respect the files boundaries as detected when [FileSet] is created
    #[test]
    fn boundaries() -> Result<()> {
        let dir = MkTemp::dir("file_set")?;
        let filepath = dir.as_path().join("test");
        let mut file = File::create(&filepath)?;

        write!(&mut file, "0123456789")?;

        let mut fileset = FileSet::new([filepath.clone(), filepath].into())?;
        /* extra data should be ignored */
        write!(&mut file, "0123456789")?;

        let mut buf = Vec::with_capacity(20);
        fileset.read_to_end(&mut buf)?;
        assert_eq!(buf, b"01234567890123456789");

        Ok(())
    }
}
