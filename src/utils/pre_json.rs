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

use std::ops::{Deref, DerefMut};

pub struct JsonPreParser {
    pub buffer: Vec<u8>,
    parser: JsonPreParserContext,
    parsed: usize,
    bytes: usize,
}

impl Default for JsonPreParser {
    fn default() -> Self {
        Self {
            buffer: Vec::with_capacity(4096),
            parser: Default::default(),
            parsed: 0,
            bytes: 0,
        }
    }
}

impl Deref for JsonPreParser {
    type Target = Vec<u8>;

    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}

impl DerefMut for JsonPreParser {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.buffer
    }
}

impl JsonPreParser {
    pub fn parse(&mut self) -> bool {
        self.flush();

        while self.parsed < self.buffer.len() {
            if let Some((size, _)) = self.parser.add(self.buffer[self.parsed]) {
                self.bytes = size;
                self.parsed += 1;

                return true;
            }
            self.parsed += 1;
        }
        /* flush garbage bytes */
        if self.parser.bytes == 0 {
            self.buffer.clear();
            self.parsed = 0;
        }
        false
    }

    #[inline]
    pub fn get(&self) -> Option<&[u8]> {
        if self.bytes != 0 {
            Some(&self.buffer[self.parsed - self.bytes..self.parsed])
        } else {
            None
        }
    }

    pub fn reset(&mut self) {
        self.buffer.clear();
        self.parser.reset();
        self.parsed = 0;
        self.bytes = 0;
    }

    fn flush(&mut self) {
        if self.bytes != 0 {
            if self.parsed < self.buffer.len() {
                self.buffer.copy_within(self.parsed.., 0);
                self.buffer.truncate(self.buffer.len() - self.parsed);
                self.parsed = 0;
            } else {
                self.buffer.clear();
            }
            self.bytes = 0;
        }
    }
}

/// JSon pre-parser
///
/// detects JSon object or array boundaries in a stream of bytes
#[derive(Debug, Default)]
struct JsonPreParserContext {
    /// Object/Array depth
    depth: usize,
    /// Skipping a character (in-string)
    skip: bool,
    /// Parsing a string
    in_string: bool,
    /// Detected object type
    obj_type: Option<JsonType>,
    /// Size of object in bytes
    bytes: usize,
}

#[derive(Eq, PartialEq, Copy, Clone, Debug)]
enum JsonType {
    Object,
    Array,
}

impl JsonPreParserContext {
    /// add a byte to the pre-parser
    ///
    /// returns bytes size and object type when object boundaries are detected
    pub fn add(&mut self, value: u8) -> Option<(usize, JsonType)> {
        if self.skip {
            self.skip = false;
        } else if let Some(obj_type) = self.obj_type {
            if self.in_string {
                match value {
                    b'"' => self.in_string = false,
                    b'\\' => self.skip = true,
                    _ => (/* string data */),
                }
            } else {
                if value == b'"' {
                    self.in_string = true;
                } else if obj_type == JsonType::Array {
                    if value == b'[' {
                        self.depth += 1;
                    } else if value == b']' {
                        self.depth -= 1;
                        return self.try_take();
                    }
                } else if obj_type == JsonType::Object {
                    if value == b'{' {
                        self.depth += 1;
                    } else if value == b'}' {
                        self.depth -= 1;
                        return self.try_take();
                    }
                }
            }
        } else {
            /* no type detected (yet) */
            match value {
                b'{' => self.obj_type = Some(JsonType::Object),
                b'[' => self.obj_type = Some(JsonType::Array),
                _ => return None, /* value dropped */
            }
            self.depth = 1;
        }

        self.bytes += 1;
        None
    }

    pub fn reset(&mut self) {
        self.depth = 0;
        self.obj_type = None;
        self.in_string = false;
        self.skip = false;
        self.bytes = 0;
    }

    fn try_take(&mut self) -> Option<(usize, JsonType)> {
        if self.depth == 0 {
            let ret = (self.bytes + 1, self.obj_type.take().unwrap());
            self.bytes = 0;
            Some(ret)
        } else {
            self.bytes += 1;
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use std::io::Write;

    use super::*;

    #[test]
    fn simple() -> Result<()> {
        let mut parser = JsonPreParser::default();
        parser.write_all(br#"  { "sample": "{{{{\"" }{}"#)?;

        assert!(parser.parse());
        assert_eq!(parser.get(), Some(br#"{ "sample": "{{{{\"" }"#.as_slice()));
        assert!(parser.parse());
        assert_eq!(parser.get(), Some(b"{}".as_slice()));
        assert_eq!(parser.len(), 2);

        assert!(!parser.parse());
        assert_eq!(parser.get(), None);
        assert_eq!(parser.len(), 0);
        Ok(())
    }

    #[test]
    fn partial() -> Result<()> {
        let mut parser = JsonPreParser::default();

        parser.write_all(b"{ \"par")?;
        assert!(!parser.parse());
        assert_eq!(parser.get(), None);

        parser.write_all(b"tial\":")?;
        assert!(!parser.parse());

        parser.write_all(b" \"item\" ")?;
        assert!(!parser.parse());

        parser.write_all(b"}")?;
        assert!(parser.parse());
        assert_eq!(parser.get(), Some(br#"{ "partial": "item" }"#.as_slice()));
        Ok(())
    }

    #[test]
    fn nested() -> Result<()> {
        let mut parser = JsonPreParser::default();
        let data = br#"{ "array": [ { "obj": "test" }, [ ], 12, true, false, "test" ], "bool": true, "sub": { } }"#;
        parser.write_all(data)?;
        assert!(parser.parse());
        assert_eq!(parser.get(), Some(data.as_slice()));
        Ok(())
    }

    #[test]
    /// ignore non objects or array
    fn ignore() -> Result<()> {
        let mut parser = JsonPreParser::default();

        for value in [b"1234".as_slice(), b"\"string\"", b"null", b"false"] {
            parser.write_all(value)?;
            assert!(!parser.parse());
        }
        assert!(!parser.parse());
        assert_eq!(parser.len(), 0);
        Ok(())
    }
}
