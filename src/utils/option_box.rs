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

pub trait OptionBoxTrait<T> {
    fn as_opt(&self) -> Option<&T>;
}

impl<T> OptionBoxTrait<T> for Option<Box<T>> {
    #[inline]
    fn as_opt(&self) -> Option<&T> {
        self.as_ref().map(AsRef::as_ref)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_opt() {
        let mut value = Some(Box::new(42u32));

        if !matches!(value.as_opt(), Some(&42u32)) {
            panic!("failed to match as Option<&T>::Some");
        }

        value = None;
        if value.as_opt().is_some() {
            panic!("failed to match as Option<&T>::None");
        }
    }
}
