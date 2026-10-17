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
** Created on: 2026-01-08T09:54:09
** Author: Sylvain Fargier <fargier.sylvain@gmail.com>
*/

use std::{
    cmp::Ordering,
    collections::BinaryHeap,
    ops::DerefMut,
    sync::{Mutex, MutexGuard},
    time::{Duration, Instant},
};

use chrono::{DateTime, DurationRound, Local, TimeDelta};
use serde::{Deserialize, Serialize};

use crate::{
    service::{Service, ServiceId},
    utils::serializers::{self, instant::from_systime},
};

#[derive(Default, Debug)]
pub struct Scheduler {
    _queue: Mutex<BinaryHeap<SchedulerEvent>>,
}

#[derive(Eq, Debug, Serialize, Deserialize, Clone)]
pub enum SchedulerEvent {
    ServiceSchedule {
        id: ServiceId,
        #[serde(with = "serializers::instant")]
        instant: Instant,
        #[serde(skip)]
        date_time: DateTime<Local>,
    },
    ServiceRestart {
        id: ServiceId,
        #[serde(with = "serializers::instant")]
        instant: Instant,
    },
    WatchServiceRestart {
        id: ServiceId,
        #[serde(with = "serializers::instant")]
        instant: Instant,
    },
    Sysinfo {
        #[serde(with = "serializers::instant")]
        instant: Instant,
    },
    ClockCheck {
        #[serde(with = "serializers::instant")]
        instant: Instant,
    },
}

impl SchedulerEvent {
    pub fn id(&self) -> Option<ServiceId> {
        match self {
            Self::ServiceSchedule { id, .. }
            | Self::ServiceRestart { id, .. }
            | Self::WatchServiceRestart { id, .. } => Some(*id),
            _ => None,
        }
    }

    pub fn instant(&self) -> &Instant {
        match self {
            Self::ServiceSchedule { instant, .. }
            | Self::ServiceRestart { instant, .. }
            | Self::WatchServiceRestart { instant, .. } => instant,
            Self::Sysinfo { instant } | Self::ClockCheck { instant } => instant,
        }
    }

    pub fn instant_mut(&mut self) -> &mut Instant {
        match self {
            SchedulerEvent::ServiceSchedule { instant, .. }
            | SchedulerEvent::ServiceRestart { instant, .. }
            | SchedulerEvent::WatchServiceRestart { instant, .. } => instant,
            SchedulerEvent::Sysinfo { instant } | SchedulerEvent::ClockCheck { instant } => instant,
        }
    }

    pub fn is_source_eq(&self, other: &SchedulerEvent) -> bool {
        match self {
            Self::ServiceSchedule { id, .. } => {
                matches!(other, Self::ServiceSchedule { id: other_id, .. } if id == other_id)
            }
            Self::ServiceRestart { id, .. } => {
                matches!(other, Self::ServiceRestart { id: other_id, .. } if id == other_id)
            }
            Self::WatchServiceRestart { id, .. } => {
                matches!(other, Self::WatchServiceRestart { id: other_id, .. } if id == other_id)
            }
            Self::Sysinfo { .. } => {
                matches!(other, Self::Sysinfo { .. })
            }
            Self::ClockCheck { .. } => {
                matches!(other, Self::ClockCheck { .. })
            }
        }
    }

    /// reschedule the event in the future (exclusive with regards to `now`), using the given interval
    pub fn reschedule(&mut self, interval: Duration) {
        let mut now = Instant::now();
        let new = self.instant_mut();

        while new <= &mut now {
            *new += interval;
        }
    }
}

impl Ord for SchedulerEvent {
    /// the lower the Instant the higher the priority
    fn cmp(&self, other: &Self) -> Ordering {
        self.instant().cmp(other.instant()).reverse()
    }
}

impl PartialOrd for SchedulerEvent {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for SchedulerEvent {
    fn eq(&self, other: &Self) -> bool {
        self.instant().eq(other.instant())
    }
}

impl Scheduler {
    #[tracing::instrument(skip(self))]
    pub fn clear(&self) {
        self.queue().clear()
    }

    /// Refresh [DateTime] based events according to eventual clock-drift
    #[tracing::instrument(skip(self))]
    pub fn refresh(&self) {
        let mut queue = self.queue();
        if !queue
            .iter()
            .any(|e| matches!(e, SchedulerEvent::ServiceSchedule { .. }))
        {
            return;
        }

        let mut old = BinaryHeap::with_capacity(queue.len());
        std::mem::swap(queue.deref_mut(), &mut old);

        for mut event in old.into_iter() {
            if let SchedulerEvent::ServiceSchedule {
                instant, date_time, ..
            } = &mut event
            {
                *instant = from_systime(&(*date_time).into());
            }
            queue.push(event);
        }
    }

    /// (re)schedule a periodic service
    ///
    /// - returns true if the enqueued item is the most prioritary in the queue
    #[tracing::instrument(fields(id = service.id, name = service.name), skip(self, service))]
    pub fn reschedule(&self, service: &Service) -> bool {
        let schedule = match service.schedule.as_ref() {
            Some(s) => s,
            None => return false,
        };
        if let Ok(next) = schedule.find_next_occurrence(&Self::now(), false) {
            tracing::info!(?next);
            self.enqueue(SchedulerEvent::ServiceSchedule {
                id: service.id,
                date_time: next,
                instant: from_systime(&next.into()),
            })
        } else {
            tracing::error!("failed to get schedule for service");
            false
        }
    }

    pub fn peek(&self) -> Option<Duration> {
        self.queue()
            .peek()
            .map(|event| event.instant().saturating_duration_since(Instant::now()))
    }

    /// enqueue a [SchedulerEvent]
    ///
    /// - All [SchedulerEvent] from the same source (identical type and id) are
    ///   removed before this one is injected
    /// - returns true if the enqueued item is the most prioritary in the queue
    #[tracing::instrument(level = "TRACE", skip(self), ret)]
    pub fn enqueue(&self, event: SchedulerEvent) -> bool {
        let stamp = *event.instant();
        let mut queue = self.queue();
        queue.retain(|evt| !evt.is_source_eq(&event));
        queue.push(event);
        queue.peek().is_some_and(|e| e.instant() == &stamp)
    }

    #[tracing::instrument(level = "TRACE", skip(self), ret)]
    pub fn remove(&self, service: &ServiceId) {
        self.queue()
            .retain(|evt| evt.id().is_none_or(|id| &id != service));
    }

    #[inline]
    fn queue<'a>(&'a self) -> MutexGuard<'a, BinaryHeap<SchedulerEvent>> {
        self._queue.lock().unwrap()
    }

    pub fn iter<'a>(&'a self) -> SchedulerIterator<'a> {
        SchedulerIterator(self, Instant::now())
    }

    pub fn dump(&self) -> Vec<SchedulerEvent> {
        let mut ret: Vec<_> = self.queue().iter().cloned().collect();
        /* BTreeMap iteration is random order */
        ret.sort_unstable_by(|x, y| y.cmp(x));
        ret
    }

    pub fn now() -> DateTime<Local> {
        Local::now().duration_trunc(TimeDelta::seconds(1)).unwrap()
    }
}

pub struct SchedulerIterator<'a>(&'a Scheduler, Instant);

impl Iterator for SchedulerIterator<'_> {
    type Item = SchedulerEvent;

    #[tracing::instrument(level = "TRACE", skip(self), ret)]
    fn next(&mut self) -> Option<Self::Item> {
        let mut queue = self.0._queue.lock().unwrap();
        if queue.peek().is_some_and(|d| d.instant() <= &self.1) {
            queue.pop()
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::{Result, anyhow};
    use std::time::{Duration, Instant};

    #[test]
    fn scheduler() -> Result<()> {
        let sched = Scheduler::default();

        sched.enqueue(SchedulerEvent::ServiceRestart {
            instant: Instant::now() + Duration::from_mins(1),
            id: 42,
        });
        sched.enqueue(SchedulerEvent::Sysinfo {
            instant: Instant::now() - Duration::from_mins(1),
        });
        let i = Instant::now() - Duration::from_mins(1);
        assert!(i < Instant::now());
        sched.enqueue(SchedulerEvent::ServiceRestart {
            instant: Instant::now() + Duration::from_mins(1),
            id: 43,
        });

        let expired: Vec<SchedulerEvent> = sched.iter().collect();
        assert_eq!(1, expired.len());
        match expired.first().unwrap() {
            SchedulerEvent::Sysinfo { .. } => {}
            _ => Err(anyhow!("should be a sysinfo"))?,
        };
        Ok(())
    }
}
