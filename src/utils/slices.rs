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

use std::borrow::Borrow;

pub trait SliceIncludes<T> {
    fn includes<N>(&self, needle: N) -> bool
    where
        N: AsRef<[T]>;

    fn includes_all<'a, I, N>(&self, needles: I) -> bool
    where
        N: 'a,
        I: Borrow<[N]>,
        N: AsRef<[T]>;
}

impl<T> SliceIncludes<T> for [T]
where
    T: PartialEq,
    T: std::fmt::Debug,
{
    fn includes<N>(&self, needle: N) -> bool
    where
        N: AsRef<[T]>,
    {
        let needle = needle.as_ref();
        let first = match needle.first() {
            Some(elt) => elt,
            None => return true,
        };

        for (pos, elt) in self.iter().enumerate() {
            if elt == first && self[pos..].starts_with(needle) {
                return true;
            }
        }
        false
    }

    fn includes_all<'a, I, N>(&self, needles: I) -> bool
    where
        N: 'a,
        I: Borrow<[N]>,
        N: AsRef<[T]>,
    {
        let mut needles: Vec<(&[T], &T)> = needles
            .borrow()
            .iter()
            .filter_map(|e| {
                let e = e.as_ref();
                if e.is_empty() {
                    None
                } else {
                    Some((e, e.first().unwrap()))
                }
            })
            .collect();

        for (pos, elt) in self.iter().enumerate() {
            needles.retain(|n| !(n.1 == elt && self[pos..].starts_with(n.0)));
            if needles.is_empty() {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slice_includes() {
        let test: &[u8] = b"this is a tttest";

        assert!(test.includes(b"this"));
        assert!(test.includes(b"ttest"));
        assert!(test.includes(b"a"));
        assert!(test.includes(b""));
        assert!(!test.includes(b"not"));

        assert!(test.includes_all([b"this", b"ttest" as &[u8]]));
        assert!(test.includes_all([b"this", b"" as &[u8]]));
    }
}
