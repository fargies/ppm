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

use std::fmt::{Debug, Formatter, Result};

/// Print an iterable item
///
/// This will consume the iterator, and allows to display containers in a fancy
/// way, eventually using `map`
///
/// - [IntoIterator::Item] must implement [Debug]
pub struct DebugIter<T>(std::cell::Cell<Option<T>>);

impl<T> Debug for DebugIter<T>
where
    T: IntoIterator,
    T::Item: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        let mut list = f.debug_list();

        if let Some(iterable) = self.0.replace(None) {
            for i in iterable.into_iter() {
                list.entry(&i);
            }
        }
        list.finish()
    }
}

impl<T> DebugIter<T> {
    pub fn new(value: T) -> Self {
        Self(std::cell::Cell::new(Some(value)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_iter() {
        let value = [1, 2, 3];
        assert_eq!(
            "[1, 2, 3]",
            format!("{:?}", DebugIter::new(value.iter())).as_str()
        );
    }
}
