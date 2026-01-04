/*
** Copyright (C) 2026 Sylvain Fargier
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
** Created on: 2026-01-04T08:06:28
** Author: Sylvain Fargier <fargier.sylvain@gmail.com>
*/

use std::sync::{
    LazyLock,
    atomic::{AtomicBool, Ordering},
};

/// Lazy loaded atomic boolean value
pub struct LazyBool(LazyLock<AtomicBool>);

impl LazyBool {
    pub const fn new(fun: fn() -> AtomicBool) -> Self {
        Self(LazyLock::<AtomicBool>::new(fun))
    }

    pub fn get(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    pub fn set(&self, value: bool) {
        self.0.store(value, Ordering::Relaxed)
    }
}
