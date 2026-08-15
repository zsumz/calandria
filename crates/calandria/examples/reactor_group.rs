//! Runs one Kafka-shaped owner singularly and as a shared-nothing reactor group.

use std::{convert::Infallible, error::Error, num::NonZeroUsize};

use calandria::{
    Completer, DrainStatus, Duty, LaneLimits, MailboxLimits, MailboxReceiver, Moment,
    MonotonicClock, Next, Reactor, ReactorGroup, ReactorGroupLimits, ReactorGroupMember,
    ReactorGroupMemberExit, ReactorGroupOutcome, ReactorId, ReactorOutcome, Retained,
    RetainedBytes, ThreadParker, Turn, WakeHandle, WorkCount, completion, mailbox, thread_parker,
};

const TURN_BUDGET: usize = 4;

#[derive(Debug)]
enum BrokerCommand {
    Produce {
        operation: u64,
        value: Vec<u8>,
        complete: Completer<Receipt>,
    },
    Shutdown,
}

impl Retained for BrokerCommand {
    fn retained_bytes(&self) -> RetainedBytes {
        match self {
            Self::Produce { value, .. } => RetainedBytes::new(
                u64::try_from(value.capacity())
                    .unwrap_or_else(|_| panic!("example payload capacity exceeds u64")),
            ),
            Self::Shutdown => RetainedBytes::ZERO,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
struct Receipt {
    operation: u64,
    reactor: ReactorId,
    bytes: usize,
}

#[derive(Debug)]
struct BrokerShard {
    id: ReactorId,
    receiver: MailboxReceiver<BrokerCommand>,
    scratch: Vec<BrokerCommand>,
    operations: u64,
    bytes: u64,
}

impl Duty for BrokerShard {
    type Error = Infallible;

    fn turn(&mut self, _now: Moment) -> Result<Turn, Self::Error> {
        self.scratch.clear();
        let report = self
            .receiver
            .drain_into(&mut self.scratch, nonzero(TURN_BUDGET));
        let mut work = report.drained();
        let mut shutdown = false;
        for command in self.scratch.drain(..) {
            shutdown |= apply_command(self.id, &mut self.operations, &mut self.bytes, command);
        }
        if shutdown {
            let pending = self.receiver.close();
            work = work.saturating_add(pending.len());
            for command in pending {
                let _ = apply_command(self.id, &mut self.operations, &mut self.bytes, command);
            }
        }
        let work = WorkCount::new(
            u64::try_from(work).unwrap_or_else(|_| panic!("example work exceeds u64")),
        );
        Ok(if shutdown || report.status() == DrainStatus::Closed {
            Turn::stopped(work)
        } else if report.status() == DrainStatus::MorePending {
            Turn::runnable(work)
        } else {
            Turn::new(work, Next::Wake)
        })
    }
}

fn apply_command(
    reactor: ReactorId,
    operations: &mut u64,
    bytes: &mut u64,
    command: BrokerCommand,
) -> bool {
    match command {
        BrokerCommand::Produce {
            operation,
            value,
            complete,
        } => {
            *operations = operations.saturating_add(1);
            *bytes = bytes.saturating_add(
                u64::try_from(value.len())
                    .unwrap_or_else(|_| panic!("example payload length exceeds u64")),
            );
            let _ = complete.complete(Receipt {
                operation,
                reactor,
                bytes: value.len(),
            });
            false
        }
        BrokerCommand::Shutdown => true,
    }
}

type Member = ReactorGroupMember<BrokerShard, MonotonicClock, ThreadParker, BrokerCommand>;
type BrokerReactor = Reactor<BrokerShard, MonotonicClock, ThreadParker>;

fn main() -> Result<(), Box<dyn Error>> {
    run_singular()?;
    run_group()?;
    Ok(())
}

fn run_singular() -> Result<(), Box<dyn Error>> {
    let (parker, notifier) = thread_parker();
    let (sender, receiver) = mailbox(mailbox_limits(), notifier.wake_handle());
    let reactor = reactor(ReactorId::new(0), receiver, parker, notifier.wake_handle());
    let handle = reactor.spawn("broker-singular")?;
    let (receipt, complete) = completion();
    sender.try_send(BrokerCommand::Produce {
        operation: 1,
        value: b"singular".to_vec(),
        complete,
    })?;
    assert_eq!(receipt.wait()?.reactor, ReactorId::new(0));
    sender.try_send_control(BrokerCommand::Shutdown)?;
    let exit = handle
        .join()
        .unwrap_or_else(|_| panic!("singular broker reactor panicked"));
    assert!(matches!(exit.outcome(), ReactorOutcome::Stopped));
    println!(
        "singular reactor processed {} operation and {} bytes",
        exit.duty().operations,
        exit.duty().bytes
    );
    Ok(())
}

fn run_group() -> Result<(), Box<dyn Error>> {
    let group = ReactorGroup::spawn(
        ReactorGroupLimits::new(nonzero(2)),
        "broker-shard",
        vec![member(ReactorId::new(0)), member(ReactorId::new(1))],
    )?;
    let ingress = group.handle();
    let (first, first_complete) = completion();
    let (second, second_complete) = completion();
    ingress.try_send(
        ReactorId::new(0),
        BrokerCommand::Produce {
            operation: 2,
            value: b"alpha".to_vec(),
            complete: first_complete,
        },
    )?;
    ingress.try_send(
        ReactorId::new(1),
        BrokerCommand::Produce {
            operation: 3,
            value: b"beta".to_vec(),
            complete: second_complete,
        },
    )?;
    assert_eq!(first.wait()?.reactor, ReactorId::new(0));
    assert_eq!(second.wait()?.reactor, ReactorId::new(1));
    for reactor in 0..2 {
        ingress.try_send_control(ReactorId::new(reactor), BrokerCommand::Shutdown)?;
    }

    let exit = group
        .join()
        .unwrap_or_else(|_| panic!("broker reactor group supervisor panicked"));
    assert_eq!(exit.outcome(), ReactorGroupOutcome::Stopped);
    for (id, member) in exit.members() {
        let ReactorGroupMemberExit::Exited(member) = member else {
            panic!("broker reactor {id:?} panicked");
        };
        println!(
            "group reactor {} processed {} operation and {} bytes",
            id.get(),
            member.duty().operations,
            member.duty().bytes
        );
    }
    Ok(())
}

fn member(id: ReactorId) -> Member {
    let (parker, notifier) = thread_parker();
    let ingress_wake = notifier.wake_handle();
    let termination_wake = notifier.wake_handle();
    ReactorGroupMember::with_mailbox(id, mailbox_limits(), ingress_wake, move |id, receiver| {
        reactor(id, receiver, parker, termination_wake)
    })
}

fn reactor(
    id: ReactorId,
    receiver: MailboxReceiver<BrokerCommand>,
    parker: ThreadParker,
    termination_wake: WakeHandle,
) -> BrokerReactor {
    let duty = BrokerShard {
        id,
        receiver,
        scratch: Vec::with_capacity(TURN_BUDGET),
        operations: 0,
        bytes: 0,
    };
    Reactor::new(duty, MonotonicClock::new(), parker, termination_wake)
}

fn mailbox_limits() -> MailboxLimits {
    MailboxLimits::new(
        LaneLimits::new(nonzero(2), RetainedBytes::ZERO),
        LaneLimits::new(nonzero(16), RetainedBytes::new(64 * 1_024)),
    )
}

fn nonzero(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap_or_else(|| panic!("example limit must be nonzero"))
}
