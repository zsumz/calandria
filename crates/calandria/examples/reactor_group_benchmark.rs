//! Measures fixed-work throughput across one, two, and four reactor-group shards.

use std::{
    convert::Infallible,
    error::Error,
    num::NonZeroUsize,
    thread,
    time::{Duration, Instant},
};

use calandria::{
    AdmissionFailure, DrainStatus, Duty, HostConfig, LaneLimits, MailboxLimits, MailboxReceiver,
    Moment, MonotonicClock, Next, Reactor, ReactorGroup, ReactorGroupLimits, ReactorGroupMember,
    ReactorGroupMemberExit, ReactorGroupSendFailure, ReactorId, Retained, RetainedBytes,
    ThreadParker, Turn, WorkCount, thread_parker,
};

const OPERATIONS: usize = 200_000;
const SAMPLES: usize = 5;
const TURN_BUDGET: usize = 256;
const MAILBOX_MESSAGES: usize = 4_096;

#[derive(Clone, Copy, Debug)]
enum Command {
    Work(u64),
    Stop,
}

impl Retained for Command {
    fn retained_bytes(&self) -> RetainedBytes {
        RetainedBytes::ZERO
    }
}

#[derive(Debug)]
struct Shard {
    receiver: MailboxReceiver<Command>,
    scratch: Vec<Command>,
    processed: u64,
    checksum: u64,
}

impl Duty for Shard {
    type Error = Infallible;

    fn turn(&mut self, _now: Moment) -> Result<Turn, Self::Error> {
        self.scratch.clear();
        let report = self
            .receiver
            .drain_into(&mut self.scratch, nonzero(TURN_BUDGET));
        let mut stopping = false;
        for command in self.scratch.drain(..) {
            match command {
                Command::Work(value) => {
                    self.processed = self.processed.saturating_add(1);
                    self.checksum = mix(self.checksum, value);
                }
                Command::Stop => stopping = true,
            }
        }
        let work = WorkCount::new(
            u64::try_from(report.drained())
                .unwrap_or_else(|_| panic!("benchmark work count must fit in u64")),
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

type Member = ReactorGroupMember<Shard, MonotonicClock, ThreadParker, Command>;

fn main() -> Result<(), Box<dyn Error>> {
    println!("shards,median_ms,operations_per_second");
    for shards in [1, 2, 4] {
        let _warmup = run(shards)?;
        let mut samples = (0..SAMPLES)
            .map(|_| run(shards))
            .collect::<Result<Vec<_>, _>>()?;
        samples.sort_unstable();
        let median = samples[SAMPLES / 2];
        let operations = u32::try_from(OPERATIONS)
            .unwrap_or_else(|_| panic!("benchmark operation count must fit in u32"));
        let rate = f64::from(operations) / median.as_secs_f64();
        println!("{shards},{:.3},{rate:.0}", median.as_secs_f64() * 1_000.0);
    }
    Ok(())
}

fn run(shards: usize) -> Result<Duration, Box<dyn Error>> {
    let group = ReactorGroup::spawn(
        ReactorGroupLimits::new(nonzero(shards)),
        "calandria-benchmark",
        (0..shards)
            .map(|position| member(ReactorId::new(fixed(position))))
            .collect(),
    )?;
    let ingress = group.handle();
    let started = Instant::now();
    let producers = (0..shards)
        .map(|position| {
            let ingress = ingress.clone();
            thread::spawn(move || {
                let count = operations_for(position, shards);
                for offset in 0..count {
                    let value = fixed(position * count + offset);
                    send(
                        &ingress,
                        ReactorId::new(fixed(position)),
                        Command::Work(value),
                    );
                }
            })
        })
        .collect::<Vec<_>>();
    for producer in producers {
        producer
            .join()
            .unwrap_or_else(|_| panic!("benchmark producer panicked"));
    }
    for position in 0..shards {
        send(&ingress, ReactorId::new(fixed(position)), Command::Stop);
    }
    let exit = group
        .join()
        .unwrap_or_else(|_| panic!("benchmark group supervisor panicked"));
    let elapsed = started.elapsed();
    let processed = exit
        .members()
        .map(|(_, member)| match member {
            ReactorGroupMemberExit::Exited(exit) => exit.duty().processed,
            ReactorGroupMemberExit::Panicked(_) => panic!("benchmark member panicked"),
        })
        .sum::<u64>();
    assert_eq!(processed, fixed(OPERATIONS));
    Ok(elapsed)
}

fn send(ingress: &calandria::ReactorGroupHandle<Command>, id: ReactorId, mut command: Command) {
    loop {
        match ingress.try_send(id, command) {
            Ok(()) => return,
            Err(error) => {
                let (returned, _, failure) = error.into_parts();
                assert!(matches!(
                    failure,
                    ReactorGroupSendFailure::Mailbox(AdmissionFailure::MessageCapacity)
                ));
                command = returned;
                thread::yield_now();
            }
        }
    }
}

fn member(id: ReactorId) -> Member {
    let (parker, notifier) = thread_parker();
    let ingress_wake = notifier.wake_handle();
    let termination_wake = notifier.wake_handle();
    ReactorGroupMember::with_mailbox(id, mailbox_limits(), ingress_wake, move |_id, receiver| {
        Reactor::with_config(
            Shard {
                receiver,
                scratch: Vec::with_capacity(TURN_BUDGET),
                processed: 0,
                checksum: 0,
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
        LaneLimits::new(nonzero(2), RetainedBytes::ZERO),
        LaneLimits::new(nonzero(MAILBOX_MESSAGES), RetainedBytes::ZERO),
    )
}

fn operations_for(position: usize, shards: usize) -> usize {
    OPERATIONS / shards + usize::from(position < OPERATIONS % shards)
}

fn mix(state: u64, value: u64) -> u64 {
    (state ^ value)
        .rotate_left(13)
        .wrapping_mul(0x9e37_79b9_7f4a_7c15)
}

fn fixed(value: usize) -> u64 {
    u64::try_from(value).unwrap_or_else(|_| panic!("benchmark value must fit in u64"))
}

fn nonzero(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap_or_else(|| panic!("benchmark limit must be nonzero"))
}
