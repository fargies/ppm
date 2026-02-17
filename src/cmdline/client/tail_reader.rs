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

use anyhow::{Result, ensure};
use std::io::{Read, Seek, SeekFrom, Write};

pub trait TailReader {
    /// read the tail of a document
    fn tail<W>(&mut self, writer: &mut W, max_lines: Option<usize>) -> Result<usize>
    where
        W: Write;
}

const TAIL_READER_BUF_SIZE: usize = 2048;

impl<R> TailReader for R
where
    R: Read + Seek,
{
    #[tracing::instrument(level = "TRACE", skip(self, writer), ret)]
    fn tail<W>(&mut self, writer: &mut W, max_lines: Option<usize>) -> Result<usize>
    where
        W: Write,
    {
        let mut buffer = vec![0; TAIL_READER_BUF_SIZE];
        let start_pos = match max_lines {
            Some(max_lines) => seek_start(self, max_lines, buffer.as_mut_slice())? as u64,
            None => 0,
        };
        self.seek(SeekFrom::Start(start_pos))?;

        let mut count = 0;
        loop {
            match self.read(buffer.as_mut_slice()) {
                Ok(0) => return Ok(count),
                Ok(n) => {
                    if n < buffer.len() {
                        writer.write_all(&buffer[..n])?;
                        return Ok(count + n);
                    } else {
                        count += n;
                        writer.write_all(&buffer)?;
                    }
                }
                Err(e) => return Err(e.into()),
            }
        }

        #[tracing::instrument(level = "TRACE", skip(reader, buffer), ret)]
        fn seek_start<R>(reader: &mut R, mut max_lines: usize, buffer: &mut [u8]) -> Result<usize>
        where
            R: Read + Seek,
        {
            /* skip last byte (may be a `\n` don't consider it as a line) */
            reader.seek(SeekFrom::End(-1))?;
            let mut remaining_size = reader.stream_position()? as usize;

            while remaining_size != 0 {
                let len = buffer.len().min(remaining_size);
                let buffer = &mut buffer[..len];
                remaining_size -= len;
                reader.seek(SeekFrom::Start(remaining_size as u64))?;
                // let len = buffer.len().min(remaining_size);
                // let buffer = &mut buffer[..len];
                // reader.seek(SeekFrom::Current(-(len as i64)))?;
                ensure!(reader.read(buffer)? == len, "failed to read");
                // remaining_size -= len;
                if let Some(buffer_pos) = rcount_lines(buffer, &mut max_lines) {
                    return Ok(remaining_size + buffer_pos + 1);
                }
            }

            Ok(0)
        }

        #[tracing::instrument(level = "TRACE", skip(buffer), ret)]
        fn rcount_lines(buffer: &[u8], max_lines: &mut usize) -> Option<usize> {
            let mut iter = buffer.iter();
            if max_lines == &0 {
                return Some(buffer.len());
            }

            while let Some(pos) = iter.rposition(|c| c == &b'\n') {
                *max_lines -= 1;
                if max_lines == &0 {
                    return Some(pos);
                }
            }
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use std::io::Cursor;

    use crate::utils::MkTemp;

    use super::*;

    #[test]
    fn basic() {
        for (input, output, lines) in [
            ("a\nb\nc", "b\nc", 2),
            ("a\nb\nc\n", "b\nc\n", 2),
            ("a\nb\nc\n", "a\nb\nc\n", 3),
            ("a\nb\nc\n", "a\nb\nc\n", 5),
            ("a\nb\nc\n", "", 0),
            ("a\nb\nc\n\n", "\n", 1),
        ] {
            let mut reader = Cursor::new(input);
            let mut writer = Vec::new();
            reader.tail(&mut writer, Some(lines)).unwrap();
            assert_eq!(
                String::from_utf8_lossy(&writer),
                output,
                "failed to compare tail({input:?}, {lines}) vs {output:?}"
            );
        }
    }

    /// There was an issue whilst seeking in file, let's ensure it's fixed
    #[test]
    fn seek_error() -> Result<()> {
        let mut file = MkTemp::file("seek_error")?;
        for _ in 0..(TAIL_READER_BUF_SIZE / 10) {
            writeln!(&mut file, "0123456789")?; /* 11 bytes */
        }
        file.seek(SeekFrom::Start(0))?;

        let mut writer: Vec<u8> = Vec::new();
        for n in 0..(TAIL_READER_BUF_SIZE / 10) {
            writer.clear();
            file.tail(&mut writer, Some(n)).unwrap();
            assert_eq!(n, writer.iter().filter(|&f| f == &b'\n').count());
            assert_eq!(11 * n, writer.len());
        }

        writer.clear();
        file.tail(&mut writer, None).unwrap();
        assert_eq!(
            TAIL_READER_BUF_SIZE / 10,
            writer.iter().filter(|&f| f == &b'\n').count()
        );
        assert_eq!(11 * (TAIL_READER_BUF_SIZE / 10), writer.len());
        Ok(())
    }
}
