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
** Created on: 2025-12-24T17:31:19
** Author: Sylvain Fargier <fargier.sylvain@gmail.com>
*/

use std::{
    collections::VecDeque,
    ops::Deref,
    panic::UnwindSafe,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
};

pub struct ThreadPool {
    core: Arc<ThreadPoolCore>,
    workers: Vec<Worker>,
}

impl Deref for ThreadPool {
    type Target = Arc<ThreadPoolCore>;

    fn deref(&self) -> &Self::Target {
        &self.core
    }
}

impl ThreadPool {
    #[tracing::instrument()]
    pub fn new(num_threads: usize) -> ThreadPool {
        let core = Arc::new(ThreadPoolCore {
            running: AtomicBool::new(true),
            cond: Condvar::new(),
            queue: Mutex::new(VecDeque::new()),
        });
        let mut workers = Vec::with_capacity(num_threads);
        for id in 1..=num_threads {
            workers.push(Worker::new(id, Arc::clone(&core)));
            tracing::trace!(id, "worker created");
        }
        ThreadPool { core, workers }
    }

    #[tracing::instrument(skip(self))]
    pub fn join(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        {
            let _guard = self.queue.lock().unwrap();
            self.cond.notify_all();
        }
        self.workers.clear();
    }
}

impl Drop for ThreadPool {
    fn drop(&mut self) {
        self.join();
    }
}

pub struct ThreadPoolCore {
    running: AtomicBool,
    cond: Condvar,
    queue: Mutex<VecDeque<Box<dyn FnOnce() + Send + UnwindSafe + 'static>>>,
}

impl ThreadPoolCore {
    #[tracing::instrument(skip(self, fun))]
    pub fn spawn<T>(self: &Arc<Self>, fun: T) -> bool
    where
        T: FnOnce() + Send + UnwindSafe + 'static,
    {
        if !self.running.load(Ordering::Relaxed) {
            tracing::error!("thread pool stopped, not spawning");
            return false;
        }
        let mut queue = self.queue.lock().unwrap();
        queue.push_back(Box::new(fun));
        self.cond.notify_one();
        true
    }
}

struct Worker(Option<JoinHandle<()>>);

impl Drop for Worker {
    fn drop(&mut self) {
        if let Some(handle) = self.0.take() {
            if let Err(error) = handle.join() {
                tracing::error!(?error, "worker error");
            }
            tracing::trace!("worker thread joined");
        }
    }
}

impl Worker {
    pub fn new(id: usize, core: Arc<ThreadPoolCore>) -> Worker {
        let join_handle = std::thread::spawn(move || {
            tracing::trace!(id, "worker thread enter");
            let mut guard = core.queue.lock().unwrap();
            loop {
                match guard.pop_front() {
                    Some(task) => {
                        drop(guard);
                        tracing::trace!(id, "worker running task");
                        if let Err(error) = std::panic::catch_unwind(task) {
                            tracing::warn!(id, ?error, "worker process panicked");
                        }
                        guard = core.queue.lock().unwrap();
                    }
                    None => {
                        if !core.running.load(Ordering::Relaxed) {
                            break;
                        }
                        tracing::trace!(id, "worker idle");
                        guard = core.cond.wait(guard).unwrap()
                    }
                }
            }
            tracing::trace!(id, "worker thread exit");
        });
        Worker(Some(join_handle))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;

    use super::*;

    #[test]
    fn basic() {
        let value = Arc::new(AtomicUsize::new(0));
        {
            let pool = ThreadPool::new(4);
            let value = Arc::clone(&value);
            pool.spawn(move || value.store(1, Ordering::Relaxed));
        }
        assert_eq!(value.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn panic() {
        let value = Arc::new(AtomicUsize::new(0));
        {
            let pool = ThreadPool::new(1);
            let value = Arc::clone(&value);
            pool.spawn(move || panic!("on purpose"));
            pool.spawn(move || value.store(1, Ordering::Relaxed));
        }
        assert_eq!(value.load(Ordering::Relaxed), 1);
    }
}
