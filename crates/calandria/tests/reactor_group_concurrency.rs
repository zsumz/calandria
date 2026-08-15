//! Concurrent reactor-group admission and idempotent termination qualification.

use std::{convert::Infallible, error::Error, num::NonZeroUsize, sync::Arc, thread};

use calandria::{
    DrainStatus, Duty, HostConfig, LaneLimits, MailboxLimits, MailboxReceiver, Moment,
    MonotonicClock, Next, Reactor, ReactorGroup, ReactorGroupLimits, ReactorGroupMember,
    ReactorGroupMemberExit, ReactorGroupOutcome, ReactorGroupSendFailure, ReactorId,
    ReactorOutcome, ReactorTerminationStatus, Retained, RetainedBytes, ThreadParker, Turn,
    WorkCount, thread_parker,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Command {
    Record(u64),
}

impl Retained for Command {
    fn retained_bytes(&self) -> RetainedBytes {
        RetainedBytes::ZERO
    }
}

#[derive(Debug)]
struct ShardDuty {
    receiver: MailboxReceiver<Command>,
    scratch: Vec<Command>,
    checksum: u64,
}

impl Duty for ShardDuty {
    type Error = Infallible;

    fn turn(&mut self, _now: Moment) -> Result<Turn, Self::Error> {
        self.scratch.clear();
        let report = self.receiver.drain_into(&mut self.scratch, nonzero(8));
        for Command::Record(value) in self.scratch.drain(..) {
            self.checksum ^= value;
        }
        let work = WorkCount::new(
            u64::try_from(report.drained())
                .unwrap_or_else(|_| panic!("test work count must fit in u64")),
        );
        Ok(if report.status() == DrainStatus::Closed {
            Turn::stopped(work)
        } else if report.status() == DrainStatus::MorePending {
            Turn::runnable(work)
        } else {
            Turn::new(work, Next::Wake)
        })
    }
}

type Member = ReactorGroupMember<ShardDuty, MonotonicClock, ThreadParker, Command>;

const REACTORS: usize = 4;
const ATTEMPTS: u64 = 128;

#[test]
fn concurrent_admission_and_termination_close_once_without_losing_ownership()
-> Result<(), Box<dyn Error>> {
    let members = (0..REACTORS)
        .map(|position| member(ReactorId::new(position as u64)))
        .collect();
    let group = Arc::new(ReactorGroup::spawn(
        group_limits(),
        "concurrent-group",
        members,
    )?);
    let ingress = group.handle();

    let senders = (0..REACTORS)
        .map(|position| {
            let ingress = ingress.clone();
            thread::spawn(move || {
                for value in 0..ATTEMPTS {
                    let command = Command::Record(value);
                    if let Err(error) = ingress.try_send(ReactorId::new(position as u64), command) {
                        assert_eq!(error.into_item(), Command::Record(value));
                    }
                }
            })
        })
        .collect::<Vec<_>>();
    let terminators = (0..2)
        .map(|_| {
            let group = Arc::clone(&group);
            thread::spawn(move || group.request_termination())
        })
        .collect::<Vec<_>>();

    for sender in senders {
        sender
            .join()
            .unwrap_or_else(|_| panic!("group sender panicked"));
    }
    let terminations = terminators
        .into_iter()
        .map(|terminator| {
            terminator
                .join()
                .unwrap_or_else(|_| panic!("group terminator panicked"))
        })
        .collect::<Vec<_>>();
    assert!(terminations.iter().all(|termination| {
        termination.iter().all(|(_, result)| {
            matches!(
                result.status(),
                ReactorTerminationStatus::Requested
                    | ReactorTerminationStatus::AlreadyRequested
                    | ReactorTerminationStatus::Exited
            )
        })
    }));

    let group = Arc::try_unwrap(group)
        .unwrap_or_else(|_| panic!("all concurrent group owners must be released"));
    let exit = group
        .join()
        .unwrap_or_else(|_| panic!("reactor group supervisor panicked"));
    assert_eq!(exit.outcome(), ReactorGroupOutcome::Terminated);
    assert!(exit.members().all(|(_, member)| matches!(
        member,
        ReactorGroupMemberExit::Exited(exit)
            if matches!(exit.outcome(), ReactorOutcome::Terminated)
    )));
    let Err(closed) = ingress.try_send(ReactorId::new(0), Command::Record(999)) else {
        panic!("terminal group must reject ingress");
    };
    assert!(matches!(closed.failure(), ReactorGroupSendFailure::Closed));
    assert_eq!(closed.into_item(), Command::Record(999));
    Ok(())
}

fn member(id: ReactorId) -> Member {
    let (parker, notifier) = thread_parker();
    let ingress_wake = notifier.wake_handle();
    let termination_wake = notifier.wake_handle();
    ReactorGroupMember::with_mailbox(id, mailbox_limits(), ingress_wake, move |_id, receiver| {
        Reactor::with_config(
            ShardDuty {
                receiver,
                scratch: Vec::with_capacity(8),
                checksum: 0,
            },
            MonotonicClock::new(),
            parker,
            termination_wake,
            HostConfig::default(),
        )
    })
}

fn group_limits() -> ReactorGroupLimits {
    ReactorGroupLimits::new(nonzero(REACTORS))
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
