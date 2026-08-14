//! Dedicated duty-host lifecycle, notification, and failure tests.

use std::{convert::Infallible, error::Error, fmt, num::NonZeroUsize, sync::mpsc, time::Duration};

use calandria::{
    DedicatedFailure, DedicatedHost, DedicatedOutcome, DrainStatus, Duty, EmbeddedHost, HostConfig,
    LaneLimits, MailboxLimits, MailboxReceiver, Moment, MonotonicClock, Retained, RetainedBytes,
    Span, Turn, WaitOutcome, Waiter, WorkCount, mailbox, thread_parker,
};

#[derive(Debug, Eq, PartialEq)]
struct Frame(Vec<u8>);

impl Retained for Frame {
    fn retained_bytes(&self) -> RetainedBytes {
        RetainedBytes::new(
            u64::try_from(self.0.len())
                .unwrap_or_else(|_| panic!("test frame length must fit the accounting domain")),
        )
    }
}

#[derive(Debug)]
struct FrameDuty {
    receiver: MailboxReceiver<Frame>,
    scratch: Vec<Frame>,
    frames: u64,
    bytes: u64,
    reports: Option<mpsc::Sender<u64>>,
}

impl FrameDuty {
    fn new(receiver: MailboxReceiver<Frame>) -> Self {
        Self {
            receiver,
            scratch: Vec::with_capacity(2),
            frames: 0,
            bytes: 0,
            reports: None,
        }
    }

    fn with_reports(receiver: MailboxReceiver<Frame>, reports: mpsc::Sender<u64>) -> Self {
        Self {
            receiver,
            scratch: Vec::with_capacity(2),
            frames: 0,
            bytes: 0,
            reports: Some(reports),
        }
    }
}

impl Duty for FrameDuty {
    type Error = Infallible;

    fn turn(&mut self, _now: Moment) -> Result<Turn, Self::Error> {
        self.scratch.clear();
        let report = self
            .receiver
            .drain_into(&mut self.scratch, nonzero_usize(2));
        let work = WorkCount::new(
            u64::try_from(report.drained())
                .unwrap_or_else(|_| panic!("drain count must fit the diagnostic domain")),
        );
        for frame in self.scratch.drain(..) {
            self.frames = self.frames.saturating_add(1);
            self.bytes = self.bytes.saturating_add(
                u64::try_from(frame.0.len())
                    .unwrap_or_else(|_| panic!("frame length must fit the diagnostic domain")),
            );
            if let Some(reports) = &self.reports {
                let _ignored = reports.send(self.frames);
            }
        }

        Ok(match report.status() {
            DrainStatus::MorePending => Turn::runnable(work),
            DrainStatus::Idle => Turn::new(work, calandria::Next::Wake),
            DrainStatus::Closed => Turn::stopped(work),
        })
    }
}

#[test]
fn dedicated_host_runs_a_non_io_frame_owner_to_terminal_closure() -> Result<(), Box<dyn Error>> {
    let (parker, notifier) = thread_parker();
    let wake = notifier.wake_handle();
    let limits = MailboxLimits::new(
        LaneLimits::new(nonzero_usize(1), RetainedBytes::new(1)),
        LaneLimits::new(nonzero_usize(8), RetainedBytes::new(1_024)),
    );
    let (sender, receiver) = mailbox(limits, wake);
    let host = EmbeddedHost::new(
        FrameDuty::new(receiver),
        MonotonicClock::new(),
        HostConfig::default(),
    );
    let dedicated = DedicatedHost::spawn("calandria-frame-owner", host, parker)?;

    assert!(sender.try_send(Frame(vec![1, 2, 3])).is_ok());
    assert!(sender.try_send(Frame(vec![4, 5])).is_ok());
    assert!(sender.try_send(Frame(vec![6])).is_ok());
    drop(sender);

    let Ok(exit) = dedicated.join() else {
        panic!("frame owner thread panicked");
    };
    assert!(matches!(exit.outcome(), DedicatedOutcome::Stopped));
    assert_eq!(exit.duty().frames, 3);
    assert_eq!(exit.duty().bytes, 6);
    assert!(exit.host_snapshot().turns() >= 2);
    let (_host, _waiter, outcome, dedicated) = exit.into_parts();
    assert!(matches!(outcome, DedicatedOutcome::Stopped));
    assert_eq!(
        dedicated.waits(),
        dedicated
            .notifications()
            .saturating_add(dedicated.idle_returns())
    );
    Ok(())
}

#[test]
fn repeated_publication_wakes_survive_owner_parking() -> Result<(), Box<dyn Error>> {
    let (parker, notifier) = thread_parker();
    let limits = MailboxLimits::new(
        LaneLimits::new(nonzero_usize(1), RetainedBytes::new(1)),
        LaneLimits::new(nonzero_usize(4), RetainedBytes::new(1_024)),
    );
    let (sender, receiver) = mailbox(limits, notifier.wake_handle());
    let (reports, observed) = mpsc::channel();
    let host = EmbeddedHost::new(
        FrameDuty::with_reports(receiver, reports),
        MonotonicClock::new(),
        HostConfig::default(),
    );
    let dedicated = DedicatedHost::spawn("calandria-repeated-wake", host, parker)?;

    assert!(sender.try_send(Frame(vec![1])).is_ok());
    assert_eq!(observed.recv_timeout(Duration::from_secs(2))?, 1);
    assert!(sender.try_send(Frame(vec![2])).is_ok());
    assert_eq!(observed.recv_timeout(Duration::from_secs(2))?, 2);
    drop(sender);

    let Ok(exit) = dedicated.join() else {
        panic!("repeated-wake owner thread panicked");
    };
    assert!(matches!(exit.outcome(), DedicatedOutcome::Stopped));
    assert_eq!(exit.duty().frames, 2);
    Ok(())
}

#[derive(Debug)]
struct WaitingDuty;

impl Duty for WaitingDuty {
    type Error = Infallible;

    fn turn(&mut self, _now: Moment) -> Result<Turn, Self::Error> {
        Ok(Turn::waiting())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct WaitFailure;

impl fmt::Display for WaitFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("planned wait failure")
    }
}

impl Error for WaitFailure {}

#[derive(Debug)]
struct FailingWaiter;

impl Waiter<WaitingDuty> for FailingWaiter {
    type Error = WaitFailure;

    fn wait(
        &mut self,
        _duty: &mut WaitingDuty,
        _maximum: Span,
    ) -> Result<WaitOutcome, Self::Error> {
        Err(WaitFailure)
    }
}

#[test]
fn waiting_failure_returns_the_owned_terminal_duty() -> Result<(), Box<dyn Error>> {
    let host = EmbeddedHost::new(WaitingDuty, MonotonicClock::new(), HostConfig::default());
    let dedicated = DedicatedHost::spawn("calandria-failing-wait", host, FailingWaiter)?;
    let Ok(exit) = dedicated.join() else {
        panic!("failing waiter thread panicked");
    };

    assert!(matches!(
        exit.outcome(),
        DedicatedOutcome::Failed(DedicatedFailure::Wait(WaitFailure))
    ));
    assert_eq!(exit.host_snapshot().phase(), calandria::HostPhase::Failed);
    assert_eq!(exit.dedicated_snapshot().waits(), 0);
    Ok(())
}

fn nonzero_usize(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap_or_else(|| panic!("test limit must be nonzero"))
}
