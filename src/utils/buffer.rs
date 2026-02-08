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

#![allow(dead_code)]

use std::{
    fmt::Debug,
    ops::{Range, RangeBounds},
};

pub const DEFAULT_BUFFER_SIZE: usize = 2048;

#[derive(Eq, PartialEq, Clone)]
pub struct Buffer {
    data: Vec<u8>,
    range: Range<usize>,
}

impl Default for Buffer {
    fn default() -> Self {
        Self {
            data: vec![0; DEFAULT_BUFFER_SIZE],
            range: Default::default(),
        }
    }
}

impl Debug for Buffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Buffer")
            .field("content", &self.as_slice())
            .field("range", &self.range)
            .finish()
    }
}

impl Buffer {
    pub fn new(size: usize) -> Self {
        Self {
            data: vec![0; size],
            range: Default::default(),
        }
    }

    pub fn as_slice(&self) -> &[u8] {
        /* range is always in bounds */
        unsafe { self.data.get_unchecked(self.range.clone()) }
    }

    /// Get the full raw buffer
    pub fn raw(&mut self) -> &mut [u8] {
        self.data.as_mut_slice()
    }

    pub fn consume(&mut self, count: usize) -> &mut Self {
        self.range.start += count;
        if self.range.start > self.range.end {
            self.range.start = self.range.end;
        };
        self
    }

    pub fn reset(&mut self) -> &mut Self {
        self.range = Default::default();
        self
    }

    pub fn set_range<T>(&mut self, range: T) -> &mut Self
    where
        T: RangeBounds<usize>,
    {
        self.range = Range {
            start: match range.start_bound() {
                std::ops::Bound::Included(x) => *x.min(&self.capacity()),
                std::ops::Bound::Excluded(x) => (x + 1).min(self.capacity()),
                std::ops::Bound::Unbounded => 0,
            },
            end: match range.end_bound() {
                std::ops::Bound::Included(x) => (x + 1).min(self.capacity()),
                std::ops::Bound::Excluded(x) => *x.min(&self.capacity()),
                std::ops::Bound::Unbounded => self.capacity(),
            },
        };
        self
    }

    pub fn is_empty(&self) -> bool {
        self.range.is_empty()
    }

    pub fn len(&self) -> usize {
        self.range.len()
    }

    pub fn capacity(&self) -> usize {
        self.data.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use std::io::Write;

    #[test]
    fn buffer() -> Result<()> {
        let mut buffer = Buffer::default();
        write!(buffer.raw(), "test")?;
        assert_eq!(b"", buffer.as_slice());
        assert!(buffer.is_empty());
        buffer.set_range(..4);
        assert!(!buffer.is_empty());
        assert_eq!(4, buffer.len());

        {
            let mut another = buffer.clone();
            assert_eq!(another, buffer);
            another.set_range(0..4);
            assert_eq!(another, buffer);

            another.consume(1);
            buffer.set_range(1..=3);
            assert_eq!(another, buffer);
        }

        buffer.set_range(..4);
        assert_eq!(b"test", buffer.as_slice());
        buffer.consume(2);

        assert_eq!(b"st", buffer.as_slice());
        buffer.consume(2);
        assert_eq!(b"", buffer.as_slice());

        buffer.set_range(0..0);
        assert_eq!(b"", buffer.as_slice());

        buffer.set_range(buffer.capacity()..buffer.capacity());
        assert_eq!(b"", buffer.as_slice());
        assert!(buffer.is_empty());

        buffer.set_range(..);
        assert_eq!(buffer.capacity(), buffer.len());
        Ok(())
    }
}
