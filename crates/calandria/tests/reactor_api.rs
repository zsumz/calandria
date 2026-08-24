//! Pre-run reactor ownership and lossless spawn-failure API tests.

use core::convert::Infallible;
use std::error::Error;

use calandria::{
    Clock, Duty, HostConfig, Moment, Reactor, ReactorOutcome, Span, Turn, WaitOutcome, Waiter,
    WakeHandle, WorkCount,
};

#[derive(Debug)]
struct StoppingDuty {
    work: u64,
}

impl Duty for StoppingDuty {
    type Error = Infallible;

    fn turn(&mut self, _now: Moment) -> Result<Turn, Self::Error> {
        Ok(Turn::stopped(WorkCount::new(self.work)))
    }
}

#[derive(Debug)]
struct FixedClock {
    now: Moment,
}

impl Clock for FixedClock {
    type Error = Infallible;

    fn now(&mut self) -> Result<Moment, Self::Error> {
        Ok(self.now)
    }
}

#[derive(Debug)]
struct RecordingWaiter {
    marker: u64,
}

impl Waiter<StoppingDuty> for RecordingWaiter {
    type Error = Infallible;

    fn wait(
        &mut self,
        _duty: &mut StoppingDuty,
        _maximum: Span,
    ) -> Result<WaitOutcome, Self::Error> {
        Ok(WaitOutcome::Idle)
    }
}

#[derive(Debug)]
struct WaitOnce {
    ready_to_stop: bool,
}

impl Duty for WaitOnce {
    type Error = Infallible;

    fn turn(&mut self, _now: Moment) -> Result<Turn, Self::Error> {
        if self.ready_to_stop {
            Ok(Turn::stopped(WorkCount::new(1)))
        } else {
            Ok(Turn::waiting())
        }
    }
}

#[derive(Debug)]
struct IdleOnce;

impl Waiter<WaitOnce> for IdleOnce {
    type Error = Infallible;

    fn wait(&mut self, duty: &mut WaitOnce, _maximum: Span) -> Result<WaitOutcome, Self::Error> {
        duty.ready_to_stop = true;
        Ok(WaitOutcome::Idle)
    }
}

#[test]
fn reactor_exposes_owned_components_before_execution() {
    let mut reactor = reactor();
    assert_eq!(reactor.duty().work, 1);
    reactor.duty_mut().work = 2;
    assert_eq!(reactor.waiter().marker, 3);
    reactor.waiter_mut().marker = 4;

    let (host, waiter) = reactor.into_parts();
    assert_eq!(host.duty().work, 2);
    assert_eq!(host.config(), HostConfig::new(Span::from_nanos(5)));
    assert_eq!(waiter.marker, 4);
}

#[test]
fn reactor_records_an_idle_wait_before_stopping() {
    let exit = Reactor::new(
        WaitOnce {
            ready_to_stop: false,
        },
        FixedClock {
            now: Moment::ORIGIN,
        },
        IdleOnce,
        WakeHandle::new(|| Ok(())),
    )
    .run();

    assert!(matches!(exit.outcome(), ReactorOutcome::Stopped));
    assert_eq!(exit.reactor_snapshot().waits(), 1);
    assert_eq!(exit.reactor_snapshot().notifications(), 0);
    assert_eq!(exit.reactor_snapshot().idle_returns(), 1);
}

#[test]
fn spawn_failure_reports_its_source_and_returns_all_ownership() {
    let Err(error) = reactor().spawn("invalid\0reactor-name") else {
        panic!("invalid thread name must reject startup");
    };
    assert_eq!(
        error.source_error().kind(),
        std::io::ErrorKind::InvalidInput
    );
    assert_eq!(
        error.to_string(),
        "reactor thread creation failed: thread name contains an interior null byte"
    );
    assert!(format!("{error:?}").contains("ReactorSpawnError"));
    assert_eq!(
        Error::source(&error).map(ToString::to_string),
        Some(String::from("thread name contains an interior null byte"))
    );

    let (source, reactor) = error.into_parts();
    assert_eq!(source.kind(), std::io::ErrorKind::InvalidInput);
    assert_eq!(reactor.duty().work, 1);
    assert_eq!(reactor.waiter().marker, 3);
}

fn reactor() -> Reactor<StoppingDuty, FixedClock, RecordingWaiter> {
    Reactor::with_config(
        StoppingDuty { work: 1 },
        FixedClock {
            now: Moment::ORIGIN,
        },
        RecordingWaiter { marker: 3 },
        WakeHandle::new(|| Ok(())),
        HostConfig::new(Span::from_nanos(5)),
    )
}
