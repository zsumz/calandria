//! Shared bounded reactor-group behavior fixture.

use std::{
    num::NonZeroUsize,
    sync::{Arc, Mutex, MutexGuard},
};

use calandria::{
    DrainStatus, Duty, HostConfig, LaneLimits, MailboxLimits, MailboxReceiver, Moment,
    MonotonicClock, Next, Reactor, ReactorGroupLimits, ReactorGroupMember, ReactorId, Retained,
    RetainedBytes, ThreadParker, Turn, WakeHandle, WorkCount, thread_parker,
};

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Command {
    Record(u64),
    Stop,
}

impl Retained for Command {
    fn retained_bytes(&self) -> RetainedBytes {
        RetainedBytes::ZERO
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PlannedFailure;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FirstTurn {
    Run,
    Fail,
    Panic,
}

#[derive(Debug)]
pub(crate) struct ShardDuty {
    id: ReactorId,
    receiver: MailboxReceiver<Command>,
    scratch: Vec<Command>,
    observed: Observed,
    first_turn: FirstTurn,
    turns: u64,
}

impl ShardDuty {
    pub(crate) const fn id(&self) -> ReactorId {
        self.id
    }

    fn apply_scratch(&mut self) -> bool {
        let mut stopping = false;
        for command in self.scratch.drain(..) {
            match command {
                Command::Record(value) => lock(&self.observed).push((self.id, value)),
                Command::Stop => stopping = true,
            }
        }
        stopping
    }
}

impl Duty for ShardDuty {
    type Error = PlannedFailure;

    fn turn(&mut self, _now: Moment) -> Result<Turn, Self::Error> {
        if self.turns == 0 {
            match self.first_turn {
                FirstTurn::Run => {}
                FirstTurn::Fail => {
                    self.turns = 1;
                    return Err(PlannedFailure);
                }
                FirstTurn::Panic => panic!("planned reactor panic"),
            }
        }
        self.turns = self.turns.saturating_add(1);
        self.scratch.clear();
        let report = self.receiver.drain_into(&mut self.scratch, nonzero(8));
        let mut work = report.drained();
        let mut stopping = self.apply_scratch();
        if stopping {
            self.scratch = self.receiver.close();
            work = work.saturating_add(self.scratch.len());
            stopping |= self.apply_scratch();
        }
        let work = WorkCount::new(
            u64::try_from(work).unwrap_or_else(|_| panic!("test work must fit in u64")),
        );
        if stopping || report.status() == DrainStatus::Closed {
            Ok(Turn::stopped(work))
        } else if report.status() == DrainStatus::MorePending {
            Ok(Turn::runnable(work))
        } else {
            Ok(Turn::new(work, Next::Wake))
        }
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

pub(crate) fn member(id: ReactorId, observed: &Observed, first_turn: FirstTurn) -> Member {
    let (parker, notifier) = thread_parker();
    let ingress_wake = notifier.wake_handle();
    let termination_wake = notifier.wake_handle();
    member_with_wakes(
        id,
        observed,
        first_turn,
        parker,
        ingress_wake,
        termination_wake,
    )
}

pub(crate) fn member_with_termination_wake(
    id: ReactorId,
    observed: &Observed,
    first_turn: FirstTurn,
    termination_wake: WakeHandle,
) -> Member {
    let (parker, notifier) = thread_parker();
    member_with_wakes(
        id,
        observed,
        first_turn,
        parker,
        notifier.wake_handle(),
        termination_wake,
    )
}

fn member_with_wakes(
    id: ReactorId,
    observed: &Observed,
    first_turn: FirstTurn,
    parker: ThreadParker,
    ingress_wake: WakeHandle,
    termination_wake: WakeHandle,
) -> Member {
    let observed = Arc::clone(observed);
    ReactorGroupMember::with_mailbox(id, mailbox_limits(), ingress_wake, move |id, receiver| {
        let duty = ShardDuty {
            id,
            receiver,
            scratch: Vec::with_capacity(8),
            observed,
            first_turn,
            turns: 0,
        };
        Reactor::with_config(
            duty,
            MonotonicClock::new(),
            parker,
            termination_wake,
            HostConfig::default(),
        )
    })
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
