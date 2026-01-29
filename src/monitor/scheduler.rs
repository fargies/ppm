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
    sync::{Mutex, MutexGuard},
    time::{Duration, Instant},
};

use chrono::{DateTime, DurationRound, Local, TimeDelta};
use serde::{Deserialize, Serialize};

use super::Monitor;
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
    #[tracing::instrument(skip(self, monitor))]
    pub fn init(&self, monitor: &Monitor) {
        self.queue().clear();

        let now = Local::now().duration_trunc(TimeDelta::seconds(1)).unwrap();
        for srv in monitor.services.iter() {
            self.reschedule(&srv, Some(now));
        }
        self.queue().push(SchedulerEvent::Sysinfo {
            instant: Instant::now() + monitor.stats_interval,
        });
        self.queue().push(SchedulerEvent::ClockCheck {
            instant: Instant::now() + monitor.clock_check_interval,
        });
    }

    /// (re)schedule a period service
    ///
    /// - returns true if the enqueued item is the most prioritary in the queue
    #[tracing::instrument(fields(id = service.id, name = service.name), skip(self, service))]
    pub fn reschedule(&self, service: &Service, last: Option<DateTime<Local>>) -> bool {
        if let Some(schedule) = service.schedule.as_ref() {
            if let Ok(next) = schedule.find_next_occurrence(
                &last
                    .unwrap_or_else(|| Local::now().duration_trunc(TimeDelta::seconds(1)).unwrap()),
                false,
            ) {
                tracing::info!(?next);
                return self.enqueue(SchedulerEvent::ServiceSchedule {
                    id: service.id,
                    date_time: next,
                    instant: from_systime(&next.into()),
                });
            } else {
                tracing::error!("failed to get schedule for service");
            }
        }
        false
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
    #[tracing::instrument(level = "TRACE", skip(self))]
    pub fn enqueue(&self, event: SchedulerEvent) -> bool {
        let stamp = *event.instant();
        let mut queue = self.queue();
        queue.retain(|evt| !evt.is_source_eq(&event));
        queue.push(event);
        queue.peek().is_some_and(|e| e.instant() == &stamp)
    }

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
}

pub struct SchedulerIterator<'a>(&'a Scheduler, Instant);

impl Iterator for SchedulerIterator<'_> {
    type Item = SchedulerEvent;

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
