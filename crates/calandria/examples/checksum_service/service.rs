//! Bounded checksum duty shared by singular and grouped example execution.

use std::{convert::Infallible, num::NonZeroUsize};

use calandria::{
    Completer, DrainStatus, Duty, HostConfig, LaneLimits, MailboxLimits, MailboxReceiver, Moment,
    MonotonicClock, Next, Reactor, ReactorGroupLimits, ReactorGroupMember, ReactorId, Retained,
    RetainedBytes, ThreadParker, Turn, WakeHandle, WorkCount, thread_parker,
};

const TURN_BUDGET: usize = 8;
const CONTROL_MESSAGES: usize = 2;
const WORK_MESSAGES: usize = 16;
const WORK_BYTES: u64 = 64 * 1_024;

#[derive(Debug)]
pub(crate) enum Command {
    Digest {
        request: u64,
        payload: Vec<u8>,
        complete: Completer<Receipt>,
    },
    Stop,
}

impl Retained for Command {
    fn retained_bytes(&self) -> RetainedBytes {
        match self {
            Self::Digest { payload, .. } => RetainedBytes::new(
                u64::try_from(payload.capacity())
                    .unwrap_or_else(|_| panic!("payload capacity must fit in u64")),
            ),
            Self::Stop => RetainedBytes::ZERO,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Receipt {
    pub(crate) request: u64,
    pub(crate) shard: ReactorId,
    pub(crate) checksum: u64,
}

#[derive(Debug)]
pub(crate) struct ChecksumShard {
    id: ReactorId,
    receiver: MailboxReceiver<Command>,
    scratch: Vec<Command>,
    requests: u64,
    bytes: u64,
}

impl Duty for ChecksumShard {
    type Error = Infallible;

    fn turn(&mut self, _now: Moment) -> Result<Turn, Self::Error> {
        self.scratch.clear();
        let report = self
            .receiver
            .drain_into(&mut self.scratch, nonzero(TURN_BUDGET));
        let mut work = report.drained();
        let mut stopping = self.apply_scratch();
        if stopping {
            self.scratch = self.receiver.close();
            work = work.saturating_add(self.scratch.len());
            stopping |= self.apply_scratch();
        }
        let work = WorkCount::new(
            u64::try_from(work).unwrap_or_else(|_| panic!("work count must fit in u64")),
        );
        Ok(if stopping || report.status() == DrainStatus::Closed {
            Turn::stopped(work)
        } else if report.status() == DrainStatus::MorePending {
            Turn::runnable(work)
        } else {
            Turn::new(work, Next::Wake)
        })
    }
}

impl ChecksumShard {
    fn apply_scratch(&mut self) -> bool {
        let mut stopping = false;
        for command in self.scratch.drain(..) {
            match command {
                Command::Digest {
                    request,
                    payload,
                    complete,
                } => {
                    self.requests = self.requests.saturating_add(1);
                    self.bytes = self.bytes.saturating_add(
                        u64::try_from(payload.len())
                            .unwrap_or_else(|_| panic!("payload length must fit in u64")),
                    );
                    let _ = complete.complete(Receipt {
                        request,
                        shard: self.id,
                        checksum: checksum(&payload),
                    });
                }
                Command::Stop => stopping = true,
            }
        }
        stopping
    }
}

pub(crate) type Member = ReactorGroupMember<ChecksumShard, MonotonicClock, ThreadParker, Command>;
pub(crate) type ChecksumReactor = Reactor<ChecksumShard, MonotonicClock, ThreadParker>;

pub(crate) fn member(id: ReactorId) -> Member {
    let (parker, notifier) = thread_parker();
    let ingress_wake = notifier.wake_handle();
    let termination_wake = notifier.wake_handle();
    ReactorGroupMember::with_mailbox(id, service_limits(), ingress_wake, move |id, receiver| {
        reactor(id, receiver, parker, termination_wake)
    })
}

pub(crate) fn reactor(
    id: ReactorId,
    receiver: MailboxReceiver<Command>,
    parker: ThreadParker,
    termination_wake: WakeHandle,
) -> ChecksumReactor {
    Reactor::with_config(
        ChecksumShard {
            id,
            receiver,
            scratch: Vec::with_capacity(TURN_BUDGET),
            requests: 0,
            bytes: 0,
        },
        MonotonicClock::new(),
        parker,
        termination_wake,
        HostConfig::default(),
    )
}

pub(crate) fn service_limits() -> MailboxLimits {
    MailboxLimits::new(
        LaneLimits::new(nonzero(CONTROL_MESSAGES), RetainedBytes::ZERO),
        LaneLimits::new(nonzero(WORK_MESSAGES), RetainedBytes::new(WORK_BYTES)),
    )
}

pub(crate) fn group_limits(reactors: usize) -> ReactorGroupLimits {
    ReactorGroupLimits::new(nonzero(reactors))
}

fn checksum(payload: &[u8]) -> u64 {
    payload.iter().fold(0xcbf2_9ce4_8422_2325, |state, byte| {
        (state ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

fn nonzero(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap_or_else(|| panic!("example limit must be nonzero"))
}
