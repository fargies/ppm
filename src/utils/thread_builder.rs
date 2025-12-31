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
** Created on: 2025-12-24T20:38:37
** Author: Sylvain Fargier <fargier.sylvain@gmail.com>
*/

use std::{sync::{Arc, Condvar, Mutex, Weak}, thread::JoinHandle};

pub struct ThreadBuilder {
    core: Arc<ThreadBuilderCore>
}

pub struct ThreadBuilderCore {
    threads: Mutex<Vec<Arc<WarmThread>>>
}

impl ThreadBuilderCore {
}

pub struct WarmThread {
    core: Weak<ThreadBuilderCore>,
    cond: Condvar,
    lock: Mutex<(Option<JoinHandle<()>>, Option<Box<dyn FnOnce() + Send + 'static>>)>,
}

impl WarmThread {
    pub fn new(core: &Arc<ThreadBuilderCore>) -> Arc<Self> {
        let mut wt = Arc::new(WarmThread {
            core: core.downgrade(),
            cond: Condvar::new(),
            lock: Mutex::new(None),
            join_handle: None
        });
        wt
    }
    pub fn run(self: Arc<Self>, fun: dyn FnOnce() + Send + 'static) {
        *self.lock.lock().unwrap() = Box::new(fun);
        self.
    }
}

