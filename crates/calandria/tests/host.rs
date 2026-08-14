//! Embedded duty-host scheduling and terminal-state tests.

use std::{collections::VecDeque, convert::Infallible, error::Error, fmt};

use calandria::{
    Clock, Deadline, Duty, EmbeddedHost, HostAction, HostConfig, HostError, HostPhase, Moment,
    MonotonicClock, Next, Span, Turn, WorkCount,
};

#[derive(Debug)]
struct ScriptClock {
    moments: VecDeque<Moment>,
}

impl ScriptClock {
    fn new(moments: impl IntoIterator<Item = Moment>) -> Self {
        Self {
            moments: moments.into_iter().collect(),
        }
    }
}

impl Clock for ScriptClock {
    type Error = ClockExhausted;

    fn now(&mut self) -> Result<Moment, Self::Error> {
        self.moments.pop_front().ok_or(ClockExhausted)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ClockExhausted;

impl fmt::Display for ClockExhausted {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("script clock exhausted")
    }
}

impl Error for ClockExhausted {}

#[derive(Debug)]
struct RecordingDuty {
    turn: Result<Turn, DutyFailure>,
    observed: Vec<Moment>,
}

impl RecordingDuty {
    fn successful(turn: Turn) -> Self {
        Self {
            turn: Ok(turn),
            observed: Vec::new(),
        }
    }
}

impl Duty for RecordingDuty {
    type Error = DutyFailure;

    fn turn(&mut self, now: Moment) -> Result<Turn, Self::Error> {
        self.observed.push(now);
        self.turn
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DutyFailure;

impl fmt::Display for DutyFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("planned duty failure")
    }
}

impl Error for DutyFailure {}

#[test]
fn cloned_monotonic_clocks_share_one_transferable_time_domain() {
    let clock = MonotonicClock::new();
    let clone = clock.clone();
    let independent = MonotonicClock::new();

    assert!(clock.shares_origin(&clone));
    assert!(!clock.shares_origin(&independent));
}

#[test]
fn action_planning_is_reactor_neutral() {
    let config = HostConfig::new(Span::from_nanos(100));
    let now = Moment::from_nanos(50);

    assert_eq!(
        HostAction::for_next(Next::Now, now, config),
        HostAction::Continue
    );
    assert_eq!(
        HostAction::for_next(Next::Wake, now, config),
        HostAction::Wait(Span::from_nanos(100))
    );
    assert_eq!(
        HostAction::for_next(
            Next::WakeOr(Deadline::at(Moment::from_nanos(75))),
            now,
            config,
        ),
        HostAction::Wait(Span::from_nanos(25))
    );
    assert_eq!(
        HostAction::for_next(
            Next::WakeOr(Deadline::at(Moment::from_nanos(40))),
            now,
            config,
        ),
        HostAction::Continue
    );
    assert_eq!(
        HostAction::for_next(
            Next::WakeOr(Deadline::at(Moment::from_nanos(500))),
            now,
            config,
        ),
        HostAction::Wait(Span::from_nanos(100))
    );
    assert_eq!(
        HostAction::for_next(Next::Wake, now, HostConfig::new(Span::ZERO)),
        HostAction::Continue
    );
    assert_eq!(
        HostAction::for_next(Next::Stop, now, config),
        HostAction::Stop
    );
}

#[test]
fn embedded_host_uses_post_turn_time_for_parking() {
    let duty = RecordingDuty::successful(Turn::until(
        WorkCount::new(2),
        Deadline::at(Moment::from_nanos(15)),
    ));
    let clock = ScriptClock::new([Moment::from_nanos(5), Moment::from_nanos(10)]);
    let mut host = EmbeddedHost::new(duty, clock, HostConfig::new(Span::from_nanos(100)));

    let step = host
        .step()
        .unwrap_or_else(|error| panic!("embedded turn must succeed: {error}"));

    assert_eq!(step.started_at(), Moment::from_nanos(5));
    assert_eq!(step.completed_at(), Moment::from_nanos(10));
    assert_eq!(step.action(), HostAction::Wait(Span::from_nanos(5)));
    assert_eq!(host.snapshot().turns(), 1);
    assert_eq!(host.snapshot().work(), WorkCount::new(2));
    assert_eq!(host.duty().observed.as_slice(), &[Moment::from_nanos(5)]);
    assert_eq!(host.config(), HostConfig::new(Span::from_nanos(100)));
    assert_eq!(host.clock().moments.len(), 0);
}

#[test]
fn stop_is_terminal_and_does_not_run_the_duty_twice() {
    let duty = RecordingDuty::successful(Turn::stopped(WorkCount::new(1)));
    let clock = ScriptClock::new([Moment::from_nanos(1), Moment::from_nanos(2)]);
    let mut host = EmbeddedHost::new(duty, clock, HostConfig::default());

    let step = host
        .step()
        .unwrap_or_else(|error| panic!("terminal turn must succeed: {error}"));
    assert_eq!(step.action(), HostAction::Stop);
    assert_eq!(host.snapshot().phase(), HostPhase::Stopped);

    assert!(matches!(
        host.step(),
        Err(HostError::NotRunning {
            phase: HostPhase::Stopped
        })
    ));
    assert_eq!(host.duty().observed.len(), 1);
}

#[test]
fn clock_regression_fails_after_the_mutating_turn() {
    let duty = RecordingDuty::successful(Turn::waiting());
    let clock = ScriptClock::new([Moment::from_nanos(10), Moment::from_nanos(9)]);
    let mut host = EmbeddedHost::new(duty, clock, HostConfig::default());

    assert!(matches!(
        host.step(),
        Err(HostError::ClockRegressed {
            previous,
            observed,
        }) if previous == Moment::from_nanos(10) && observed == Moment::from_nanos(9)
    ));
    assert_eq!(host.snapshot().phase(), HostPhase::Failed);
    assert_eq!(host.snapshot().turns(), 0);
    assert_eq!(host.duty().observed.as_slice(), &[Moment::from_nanos(10)]);
}

#[test]
fn initial_clock_failure_does_not_run_the_duty() {
    let duty = RecordingDuty::successful(Turn::waiting());
    let clock = ScriptClock::new([]);
    let mut host = EmbeddedHost::new(duty, clock, HostConfig::default());

    assert!(matches!(host.step(), Err(HostError::Clock(ClockExhausted))));
    assert_eq!(host.snapshot().phase(), HostPhase::Failed);
    assert!(host.duty().observed.is_empty());
}

#[test]
fn post_turn_clock_failure_returns_the_mutated_terminal_duty() {
    let duty = RecordingDuty::successful(Turn::waiting());
    let clock = ScriptClock::new([Moment::from_nanos(4)]);
    let mut host = EmbeddedHost::new(duty, clock, HostConfig::default());

    assert!(matches!(host.step(), Err(HostError::Clock(ClockExhausted))));
    assert_eq!(host.snapshot().phase(), HostPhase::Failed);
    assert_eq!(host.snapshot().turns(), 0);
    assert_eq!(host.duty().observed.as_slice(), &[Moment::from_nanos(4)]);
}

#[test]
fn duty_failure_terminally_fails_the_host() {
    let duty = RecordingDuty {
        turn: Err(DutyFailure),
        observed: Vec::new(),
    };
    let clock = ScriptClock::new([Moment::from_nanos(3)]);
    let mut host = EmbeddedHost::new(duty, clock, HostConfig::default());

    assert!(matches!(host.step(), Err(HostError::Duty(DutyFailure))));
    assert_eq!(host.snapshot().phase(), HostPhase::Failed);
    assert!(matches!(
        host.step(),
        Err(HostError::NotRunning {
            phase: HostPhase::Failed
        })
    ));
}

#[test]
fn closures_are_hostable_non_reactor_duties() {
    let mut calls = 0_u64;
    let duty = move |_now: Moment| -> Result<Turn, Infallible> {
        calls = calls.saturating_add(1);
        Ok(Turn::stopped(WorkCount::new(calls)))
    };
    let clock = ScriptClock::new([Moment::ORIGIN, Moment::ORIGIN]);
    let mut host = EmbeddedHost::new(duty, clock, HostConfig::default());

    let step = host
        .step()
        .unwrap_or_else(|error| panic!("closure duty must succeed: {error}"));
    assert_eq!(step.turn().work(), WorkCount::new(1));
}

#[derive(Debug)]
struct ExplicitComposite {
    order: Vec<&'static str>,
}

impl Duty for ExplicitComposite {
    type Error = Infallible;

    fn turn(&mut self, _now: Moment) -> Result<Turn, Self::Error> {
        self.order.push("checkpoint");
        let checkpoint = Turn::until(WorkCount::new(1), Deadline::at(Moment::from_nanos(50)));

        self.order.push("maintenance");
        let maintenance = Turn::runnable(WorkCount::new(2));
        Ok(checkpoint.merge(maintenance))
    }
}

#[test]
fn explicit_composite_preserves_owner_order_and_merges_interest() {
    let duty = ExplicitComposite { order: Vec::new() };
    let clock = ScriptClock::new([Moment::from_nanos(10), Moment::from_nanos(11)]);
    let mut host = EmbeddedHost::new(duty, clock, HostConfig::default());

    let step = host
        .step()
        .unwrap_or_else(|error| panic!("composite duty must succeed: {error}"));

    assert_eq!(host.duty().order.as_slice(), &["checkpoint", "maintenance"]);
    assert_eq!(step.turn().work(), WorkCount::new(3));
    assert_eq!(step.action(), HostAction::Continue);
}
