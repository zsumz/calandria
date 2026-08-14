//! Runs a bounded non-I/O frame owner as a singular Calandria reactor.

use std::{convert::Infallible, error::Error, io, num::NonZeroUsize};

use calandria::{
    DrainStatus, Duty, LaneLimits, MailboxLimits, MailboxReceiver, Moment, MonotonicClock, Reactor,
    ReactorOutcome, Retained, RetainedBytes, Turn, WorkCount, mailbox, thread_parker,
};

#[derive(Debug)]
struct Frame(Vec<u8>);

impl Retained for Frame {
    fn retained_bytes(&self) -> RetainedBytes {
        RetainedBytes::new(
            u64::try_from(self.0.len())
                .unwrap_or_else(|_| panic!("frame length exceeds retained-byte accounting")),
        )
    }
}

#[derive(Debug)]
struct FrameOwner {
    receiver: MailboxReceiver<Frame>,
    scratch: Vec<Frame>,
    frames: u64,
    bytes: u64,
}

impl FrameOwner {
    fn new(receiver: MailboxReceiver<Frame>) -> Self {
        Self {
            receiver,
            scratch: Vec::with_capacity(4),
            frames: 0,
            bytes: 0,
        }
    }
}

impl Duty for FrameOwner {
    type Error = Infallible;

    fn turn(&mut self, _now: Moment) -> Result<Turn, Self::Error> {
        self.scratch.clear();
        let report = self
            .receiver
            .drain_into(&mut self.scratch, nonzero_usize(4));
        let work = WorkCount::new(
            u64::try_from(report.drained())
                .unwrap_or_else(|_| panic!("drain count exceeds diagnostic accounting")),
        );

        for frame in self.scratch.drain(..) {
            self.frames = self.frames.saturating_add(1);
            self.bytes = self.bytes.saturating_add(
                u64::try_from(frame.0.len())
                    .unwrap_or_else(|_| panic!("frame length exceeds diagnostic accounting")),
            );
        }

        Ok(match report.status() {
            DrainStatus::MorePending => Turn::runnable(work),
            DrainStatus::Idle => Turn::new(work, calandria::Next::Wake),
            DrainStatus::Closed => Turn::stopped(work),
        })
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let (parker, notifier) = thread_parker();
    let ingress_wake = notifier.wake_handle();
    let limits = MailboxLimits::new(
        LaneLimits::new(nonzero_usize(1), RetainedBytes::new(1)),
        LaneLimits::new(nonzero_usize(16), RetainedBytes::new(64 * 1_024)),
    );
    let (sender, receiver) = mailbox(limits, ingress_wake);
    let reactor = Reactor::new(
        FrameOwner::new(receiver),
        MonotonicClock::new(),
        parker,
        notifier.wake_handle(),
    );
    let handle = reactor.spawn("frame-owner")?;

    sender
        .try_send(Frame(b"alpha".to_vec()))
        .map_err(|_| io::Error::other("alpha frame was rejected"))?;
    sender
        .try_send(Frame(b"beta".to_vec()))
        .map_err(|_| io::Error::other("beta frame was rejected"))?;
    drop(sender);

    let Ok(exit) = handle.join() else {
        return Err(io::Error::other("frame owner panicked").into());
    };
    if !matches!(exit.outcome(), ReactorOutcome::Stopped) {
        return Err(io::Error::other("frame owner did not stop cleanly").into());
    }

    println!(
        "processed {} frames and {} bytes in {} bounded turns",
        exit.duty().frames,
        exit.duty().bytes,
        exit.host_snapshot().turns()
    );
    Ok(())
}

fn nonzero_usize(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap_or_else(|| panic!("example limit must be nonzero"))
}
