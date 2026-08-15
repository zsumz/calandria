//! Minimal live group fixture for public API observations.

use std::{
    num::NonZeroUsize,
    sync::{Arc, Mutex, MutexGuard},
};

use calandria::{
    DrainStatus, Duty, HostConfig, LaneLimits, MailboxLimits, MailboxReceiver, Moment,
    MonotonicClock, Next, Reactor, ReactorGroupLimits, ReactorGroupMember, ReactorId, Retained,
    RetainedBytes, ThreadParker, Turn, WorkCount, thread_parker,
};

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Command {
    Record(u64),
    Retained(u64),
    Stop,
}

impl Retained for Command {
    fn retained_bytes(&self) -> RetainedBytes {
        match self {
            Self::Retained(_) => RetainedBytes::new(1),
            Self::Record(_) | Self::Stop => RetainedBytes::ZERO,
        }
    }
}

#[derive(Debug)]
pub(crate) struct ShardDuty {
    id: ReactorId,
    receiver: MailboxReceiver<Command>,
    scratch: Vec<Command>,
    observed: Observed,
}

impl Duty for ShardDuty {
    type Error = core::convert::Infallible;

    fn turn(&mut self, _now: Moment) -> Result<Turn, Self::Error> {
        self.scratch.clear();
        let report = self.receiver.drain_into(&mut self.scratch, nonzero(8));
        let mut stopping = false;
        for command in self.scratch.drain(..) {
            match command {
                Command::Record(value) | Command::Retained(value) => {
                    lock(&self.observed).push((self.id, value));
                }
                Command::Stop => stopping = true,
            }
        }
        let work = WorkCount::new(
            u64::try_from(report.drained()).unwrap_or_else(|_| panic!("test work must fit in u64")),
        );
        Ok(if stopping {
            Turn::stopped(work)
        } else if report.status() == DrainStatus::MorePending {
            Turn::runnable(work)
        } else {
            Turn::new(work, Next::Wake)
        })
    }
}

pub(crate) type Observed = Arc<Mutex<Vec<(ReactorId, u64)>>>;
pub(crate) type Member = ReactorGroupMember<ShardDuty, MonotonicClock, ThreadParker, Command>;

pub(crate) fn observations() -> Observed {
    Arc::new(Mutex::new(Vec::new()))
}

pub(crate) fn observed_values(observed: &Observed) -> Vec<(ReactorId, u64)> {
    lock(observed).clone()
}

pub(crate) fn member(id: ReactorId, observed: &Observed) -> Member {
    let (parker, notifier) = thread_parker();
    let observed = Arc::clone(observed);
    ReactorGroupMember::with_mailbox(mailbox_limits(), notifier.wake_handle(), move |receiver| {
        Reactor::with_config(
            ShardDuty {
                id,
                receiver,
                scratch: Vec::with_capacity(8),
                observed,
            },
            MonotonicClock::new(),
            parker,
            notifier.wake_handle(),
            HostConfig::default(),
        )
    })
}

pub(crate) fn measured_member(id: ReactorId, observed: &Observed) -> Member {
    let (parker, notifier) = thread_parker();
    let observed = Arc::clone(observed);
    ReactorGroupMember::with_mailbox_measure(
        mailbox_limits(),
        Command::retained_bytes,
        notifier.wake_handle(),
        move |receiver| {
            Reactor::with_config(
                ShardDuty {
                    id,
                    receiver,
                    scratch: Vec::with_capacity(8),
                    observed,
                },
                MonotonicClock::new(),
                parker,
                notifier.wake_handle(),
                HostConfig::default(),
            )
        },
    )
}

pub(crate) fn group_limits(reactors: usize) -> ReactorGroupLimits {
    ReactorGroupLimits::new(nonzero(reactors))
}

fn mailbox_limits() -> MailboxLimits {
    MailboxLimits::new(
        LaneLimits::new(nonzero(2), RetainedBytes::ZERO),
        LaneLimits::new(nonzero(8), RetainedBytes::ZERO),
    )
}

fn nonzero(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap_or_else(|| panic!("test limit must be nonzero"))
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
