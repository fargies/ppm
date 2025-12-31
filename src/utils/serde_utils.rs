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
** Created on: 2025-12-25T08:00:09
** Author: Sylvain Fargier <fargier.sylvain@gmail.com>
*/

use std::cell::Cell;

use serde::{Serialize, Serializer};

pub struct SeqWrapper<T>(Cell<Option<T>>);

impl<I, P> Serialize for SeqWrapper<I>
where
    I: IntoIterator<Item = P>,
    P: Serialize,
{
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_seq(self.0.take().unwrap())
    }
}

pub fn wrap_iterator<I, P>(i: I) -> SeqWrapper<I>
where
    I: IntoIterator<Item = P>,
{
    SeqWrapper(Cell::new(Some(i)))
}

pub struct MapWrapper<T>(Cell<Option<T>>);

impl<I, K, V> Serialize for MapWrapper<I>
where
    I: IntoIterator<Item = (K, V)>,
    K: Serialize,
    V: Serialize,
{
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_map(self.0.take().unwrap())
    }
}

pub fn wrap_map_iterator<I, P>(i: I) -> MapWrapper<I>
where
    I: IntoIterator<Item = P>,
{
    MapWrapper(Cell::new(Some(i)))
}

pub struct InnerRef<K, F, T>(pub K, pub F) where F: Fn(&K) -> &T;

impl<K, T, F> Serialize for InnerRef<K, F, T>
where
    F: Fn(&K) -> &T,
    T: Serialize,
{
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.1(&self.0).serialize(s)
    }
}
