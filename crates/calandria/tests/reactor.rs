//! Singular reactor lifecycle, notification, failure, and termination tests.

use std::{
    convert::Infallible, error::Error, fmt, io, num::NonZeroUsize, sync::mpsc, time::Duration,
};

use calandria::{
    DrainStatus, Duty, HostConfig, HostPhase, LaneLimits, MailboxLimits, MailboxReceiver, Moment,
    MonotonicClock, Reactor, ReactorFailure, ReactorOutcome, ReactorTerminationStatus, Retained,
    RetainedBytes, Span, Turn, WaitOutcome, Waiter, WakeHandle, WorkCount, mailbox, thread_parker,
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
fn reactor_runs_a_non_io_frame_owner_to_terminal_closure() -> Result<(), Box<dyn Error>> {
    let (parker, notifier) = thread_parker();
    let ingress_wake = notifier.wake_handle();
    let termination_wake = notifier.wake_handle();
    let limits = MailboxLimits::new(
        LaneLimits::new(nonzero_usize(1), RetainedBytes::new(1)),
        LaneLimits::new(nonzero_usize(8), RetainedBytes::new(1_024)),
    );
    let (sender, receiver) = mailbox(limits, ingress_wake);
    let reactor = Reactor::with_config(
        FrameDuty::new(receiver),
        MonotonicClock::new(),
        parker,
        termination_wake,
        HostConfig::default(),
    );
    let handle = reactor.spawn("calandria-frame-owner")?;

    assert!(sender.try_send(Frame(vec![1, 2, 3])).is_ok());
    assert!(sender.try_send(Frame(vec![4, 5])).is_ok());
    assert!(sender.try_send(Frame(vec![6])).is_ok());
    drop(sender);

    let Ok(exit) = handle.join() else {
        panic!("frame owner thread panicked");
    };
    assert!(matches!(exit.outcome(), ReactorOutcome::Stopped));
    assert_eq!(exit.duty().frames, 3);
    assert_eq!(exit.duty().bytes, 6);
    assert!(exit.host_snapshot().turns() >= 2);
    let (_host, _waiter, outcome, reactor) = exit.into_parts();
    assert!(matches!(outcome, ReactorOutcome::Stopped));
    assert_eq!(
        reactor.waits(),
        reactor
            .notifications()
            .saturating_add(reactor.idle_returns())
    );
    Ok(())
}

#[test]
fn repeated_publication_wakes_survive_owner_parking() -> Result<(), Box<dyn Error>> {
    let (parker, notifier) = thread_parker();
    let termination_wake = notifier.wake_handle();
    let limits = MailboxLimits::new(
        LaneLimits::new(nonzero_usize(1), RetainedBytes::new(1)),
        LaneLimits::new(nonzero_usize(4), RetainedBytes::new(1_024)),
    );
    let (sender, receiver) = mailbox(limits, notifier.wake_handle());
    let (reports, observed) = mpsc::channel();
    let reactor = Reactor::with_config(
        FrameDuty::with_reports(receiver, reports),
        MonotonicClock::new(),
        parker,
        termination_wake,
        HostConfig::default(),
    );
    let handle = reactor.spawn("calandria-repeated-wake")?;

    assert!(sender.try_send(Frame(vec![1])).is_ok());
    assert_eq!(observed.recv_timeout(Duration::from_secs(2))?, 1);
    assert!(sender.try_send(Frame(vec![2])).is_ok());
    assert_eq!(observed.recv_timeout(Duration::from_secs(2))?, 2);
    drop(sender);

    let Ok(exit) = handle.join() else {
        panic!("repeated-wake owner thread panicked");
    };
    assert!(matches!(exit.outcome(), ReactorOutcome::Stopped));
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
    let reactor = Reactor::new(
        WaitingDuty,
        MonotonicClock::new(),
        FailingWaiter,
        WakeHandle::new(|| Ok(())),
    );
    let handle = reactor.spawn("calandria-failing-wait")?;
    let Ok(exit) = handle.join() else {
        panic!("failing waiter thread panicked");
    };

    assert!(matches!(
        exit.outcome(),
        ReactorOutcome::Failed(ReactorFailure::Wait(WaitFailure))
    ));
    assert_eq!(exit.host_snapshot().phase(), HostPhase::Failed);
    assert_eq!(exit.reactor_snapshot().waits(), 0);
    Ok(())
}

#[derive(Debug)]
struct ObservedWaitingDuty {
    started: Option<mpsc::SyncSender<()>>,
}

impl Duty for ObservedWaitingDuty {
    type Error = Infallible;

    fn turn(&mut self, _now: Moment) -> Result<Turn, Self::Error> {
        if let Some(started) = self.started.take() {
            let _ = started.send(());
        }
        Ok(Turn::waiting())
    }
}

#[test]
fn termination_wakes_a_parked_reactor_and_returns_its_owner() -> Result<(), Box<dyn Error>> {
    let (parker, notifier) = thread_parker();
    let (started, observed) = mpsc::sync_channel(0);
    let reactor = Reactor::new(
        ObservedWaitingDuty {
            started: Some(started),
        },
        MonotonicClock::new(),
        parker,
        notifier.wake_handle(),
    );
    let handle = reactor.spawn("calandria-terminated-reactor")?;
    observed.recv_timeout(Duration::from_secs(2))?;

    let termination = handle.request_termination();
    assert_eq!(termination.status(), ReactorTerminationStatus::Requested);
    assert!(termination.wake_error().is_none());
    assert_eq!(
        handle.request_termination().status(),
        ReactorTerminationStatus::AlreadyRequested
    );
    let exit = handle
        .join()
        .unwrap_or_else(|_| panic!("terminated reactor panicked"));

    assert!(matches!(exit.outcome(), ReactorOutcome::Terminated));
    assert_eq!(exit.host_snapshot().phase(), HostPhase::Terminated);
    Ok(())
}

#[test]
fn failed_termination_wake_keeps_a_bounded_progress_path() -> Result<(), Box<dyn Error>> {
    let (parker, _notifier) = thread_parker();
    let reactor = Reactor::with_config(
        WaitingDuty,
        MonotonicClock::new(),
        parker,
        WakeHandle::new(|| Err(io::Error::other("planned termination wake failure"))),
        HostConfig::new(Span::from_nanos(1_000_000)),
    );
    let handle = reactor.spawn("calandria-failed-termination-wake")?;

    let termination = handle.request_termination();
    assert_eq!(termination.status(), ReactorTerminationStatus::Requested);
    assert!(termination.wake_error().is_some());
    let exit = handle
        .join()
        .unwrap_or_else(|_| panic!("bounded termination reactor panicked"));

    assert!(matches!(exit.outcome(), ReactorOutcome::Terminated));
    Ok(())
}

#[test]
fn spawn_failure_returns_the_unstarted_reactor() {
    let reactor = Reactor::new(
        WaitingDuty,
        MonotonicClock::new(),
        FailingWaiter,
        WakeHandle::new(|| Ok(())),
    );
    let Err(error) = reactor.spawn("invalid\0reactor-name") else {
        panic!("invalid thread name must reject startup");
    };
    let reactor = error.into_reactor();
    assert!(matches!(reactor.duty(), WaitingDuty));
}

fn nonzero_usize(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap_or_else(|| panic!("test limit must be nonzero"))
}
