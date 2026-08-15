//! Deterministic reactor-group aggregate outcome qualification.

use std::{
    error::Error,
    num::NonZeroUsize,
    sync::{Arc, Barrier},
};

use calandria::{
    Duty, HostConfig, LaneLimits, MailboxLimits, MailboxReceiver, Moment, MonotonicClock, Reactor,
    ReactorFailure, ReactorGroup, ReactorGroupLimits, ReactorGroupMember, ReactorGroupMemberExit,
    ReactorGroupOutcome, ReactorId, ReactorOutcome, RetainedBytes, ThreadParker, Turn,
    thread_parker,
};

#[derive(Clone, Copy, Debug)]
enum FatalTurn {
    Fail,
    Panic,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PlannedFailure;

#[derive(Debug)]
struct FatalDuty {
    _receiver: MailboxReceiver<()>,
    barrier: Arc<Barrier>,
    fatal: FatalTurn,
}

impl Duty for FatalDuty {
    type Error = PlannedFailure;

    fn turn(&mut self, _now: Moment) -> Result<Turn, Self::Error> {
        self.barrier.wait();
        match self.fatal {
            FatalTurn::Fail => Err(PlannedFailure),
            FatalTurn::Panic => panic!("planned synchronized reactor panic"),
        }
    }
}

type Member = ReactorGroupMember<FatalDuty, MonotonicClock, ThreadParker, ()>;

#[test]
fn aggregate_fatal_outcome_uses_stable_severity_and_identity_order() -> Result<(), Box<dyn Error>> {
    let barrier = Arc::new(Barrier::new(3));
    let members = vec![
        member(ReactorId::new(0), barrier.clone(), FatalTurn::Fail),
        member(ReactorId::new(1), barrier.clone(), FatalTurn::Panic),
        member(ReactorId::new(2), barrier, FatalTurn::Panic),
    ];
    let group = ReactorGroup::spawn(ReactorGroupLimits::new(nonzero(3)), "fatal-order", members)?;
    let exit = group
        .join()
        .unwrap_or_else(|_| panic!("reactor group supervisor panicked"));

    assert_eq!(
        exit.outcome(),
        ReactorGroupOutcome::Panicked(ReactorId::new(1))
    );
    let Some(ReactorGroupMemberExit::Exited(failed)) = exit.member(ReactorId::new(0)) else {
        panic!("typed failure must preserve the owned reactor exit");
    };
    assert!(matches!(
        failed.outcome(),
        ReactorOutcome::Failed(ReactorFailure::Host(_))
    ));
    for reactor in [ReactorId::new(1), ReactorId::new(2)] {
        assert!(matches!(
            exit.member(reactor),
            Some(ReactorGroupMemberExit::Panicked(_))
        ));
    }
    Ok(())
}

fn member(id: ReactorId, barrier: Arc<Barrier>, fatal: FatalTurn) -> Member {
    let (parker, notifier) = thread_parker();
    let ingress_wake = notifier.wake_handle();
    let termination_wake = notifier.wake_handle();
    ReactorGroupMember::with_mailbox(id, mailbox_limits(), ingress_wake, move |_id, receiver| {
        Reactor::with_config(
            FatalDuty {
                _receiver: receiver,
                barrier,
                fatal,
            },
            MonotonicClock::new(),
            parker,
            termination_wake,
            HostConfig::default(),
        )
    })
}

fn mailbox_limits() -> MailboxLimits {
    MailboxLimits::new(
        LaneLimits::new(nonzero(1), RetainedBytes::ZERO),
        LaneLimits::new(nonzero(1), RetainedBytes::ZERO),
    )
}

fn nonzero(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap_or_else(|| panic!("test limit must be nonzero"))
}
