//! Simulation limit, lifecycle, run-outcome, and kernel diagnostic tests.

use core::{
    convert::Infallible,
    num::{NonZeroU64, NonZeroUsize},
};
use std::panic::{AssertUnwindSafe, catch_unwind};

use calandria::{Deadline, Moment, RetainedBytes, Turn, WorkCount};
use calandria_sim::{
    ActionContext, Delivery, DutyId, KernelFailure, LimitFailure, Model, RunEnd, RunError,
    Simulation, SimulationBuildError, SimulationLimits, SimulationPhase, Step, StepError, Timeline,
    TimelineId, TimelineLimits, Topology,
};

#[derive(Clone, Copy, Debug)]
enum Behavior {
    Runnable,
    Waiting,
    Stopping,
    Deadline,
    Panic,
}

#[derive(Debug)]
struct World(Behavior);

impl Model for World {
    type Event = ();
    type Observation = ();
    type Error = Infallible;

    fn turn(
        &mut self,
        _duty: DutyId,
        _now: Moment,
        _context: &mut ActionContext<'_, Self::Event, Self::Observation>,
    ) -> Result<Turn, Self::Error> {
        Ok(match self.0 {
            Behavior::Runnable => Turn::runnable(WorkCount::new(1)),
            Behavior::Waiting => Turn::waiting(),
            Behavior::Stopping => Turn::stopped(WorkCount::new(1)),
            Behavior::Deadline => {
                Turn::until(WorkCount::ZERO, Deadline::at(Moment::from_nanos(10)))
            }
            Behavior::Panic => panic!("planned lifecycle poison"),
        })
    }

    fn deliver(
        &mut self,
        _duty: DutyId,
        _delivery: Delivery<Self::Event>,
        _context: &mut ActionContext<'_, Self::Event, Self::Observation>,
    ) -> Result<Turn, Self::Error> {
        Ok(Turn::waiting())
    }
}

#[test]
fn total_action_limit_fails_terminally_and_reports_inactive_afterward() {
    let limits = SimulationLimits::default().with_total_actions(nonzero_u64(1));
    let mut simulation = simulation(Behavior::Runnable, limits);
    simulation
        .step()
        .unwrap_or_else(|error| panic!("first action must fit: {error}"));

    let Err(error) = simulation.step() else {
        panic!("second action must exceed the total limit");
    };
    assert_eq!(error.to_string(), "total action limit of 1 was reached");
    assert!(matches!(
        error,
        StepError::Limit(LimitFailure::TotalActions { limit }) if limit == nonzero_u64(1)
    ));
    assert_eq!(simulation.phase(), SimulationPhase::Failed);

    let Err(error) = simulation.step() else {
        panic!("failed simulation must stay inactive");
    };
    assert!(matches!(
        error,
        StepError::Inactive(SimulationPhase::Failed)
    ));
    assert_eq!(error.to_string(), "simulation is Failed");
}

#[test]
fn completed_and_poisoned_worlds_reject_further_steps_exactly() {
    let mut completed = simulation(Behavior::Stopping, SimulationLimits::default());
    let report = completed
        .run_to_completion()
        .unwrap_or_else(|error| panic!("stopping world must complete: {error}"));
    assert_eq!(report.end(), RunEnd::Completed);
    assert_eq!(report.snapshot().phase(), SimulationPhase::Completed);
    let Err(error) = completed.step() else {
        panic!("completed simulation must reject another step");
    };
    assert!(matches!(
        error,
        StepError::Inactive(SimulationPhase::Completed)
    ));

    let mut poisoned = simulation(Behavior::Panic, SimulationLimits::default());
    assert!(catch_unwind(AssertUnwindSafe(|| poisoned.step())).is_err());
    assert!(poisoned.is_poisoned());
    let Err(error) = poisoned.step() else {
        panic!("poisoned simulation must reject another step");
    };
    assert!(matches!(error, StepError::Poisoned));
    assert_eq!(error.to_string(), "simulation is poisoned");
}

#[test]
fn run_outcomes_distinguish_quiescence_deadlock_and_time_advance() {
    let mut quiescent = simulation(Behavior::Waiting, SimulationLimits::default());
    let report = quiescent
        .run_to_quiescence()
        .unwrap_or_else(|error| panic!("waiting world must quiesce: {error}"));
    assert_eq!(report.end(), RunEnd::Quiescent);
    assert_eq!(report.snapshot().duties(), 1);
    assert_eq!(report.snapshot().stopped(), 0);

    let mut deadlocked = simulation(Behavior::Waiting, SimulationLimits::default());
    let Err(error) = deadlocked.run_to_completion() else {
        panic!("waiting world must deadlock under completion semantics");
    };
    assert!(matches!(error, RunError::Deadlock(snapshot) if snapshot.duties() == 1));
    assert_eq!(
        error.to_string(),
        "simulation deadlocked at 0ns with 1 live duties"
    );

    let mut deadline = simulation(Behavior::Deadline, SimulationLimits::default());
    assert!(matches!(deadline.step(), Ok(Step::Action(_))));
    let Ok(Step::TimeAdvanced { from, to }) = deadline.step() else {
        panic!("waiting deadline must advance virtual time");
    };
    assert_eq!(from, Moment::ORIGIN);
    assert_eq!(to, Moment::from_nanos(10));
}

#[test]
fn configured_limits_and_build_rejections_are_exact() {
    let timeline = TimelineLimits::new(nonzero_usize(11), RetainedBytes::new(12));
    let limits = SimulationLimits::new(timeline)
        .with_max_duties(nonzero_usize(2))
        .with_effects_per_action(nonzero_usize(3))
        .with_effect_bytes_per_action(RetainedBytes::new(4))
        .with_observations_per_action(nonzero_usize(5))
        .with_observation_bytes_per_action(RetainedBytes::new(6))
        .with_total_actions(nonzero_u64(7))
        .with_actions_per_moment(nonzero_u64(8))
        .with_max_virtual_time(Moment::from_nanos(9));
    assert_eq!(limits.max_duties(), nonzero_usize(2));
    assert_eq!(limits.timeline(), timeline);
    assert_eq!(limits.effects_per_action(), nonzero_usize(3));
    assert_eq!(limits.effect_bytes_per_action(), RetainedBytes::new(4));
    assert_eq!(limits.observations_per_action(), nonzero_usize(5));
    assert_eq!(limits.observation_bytes_per_action(), RetainedBytes::new(6));
    assert_eq!(limits.total_actions(), nonzero_u64(7));
    assert_eq!(limits.actions_per_moment(), nonzero_u64(8));
    assert_eq!(limits.max_virtual_time(), Moment::from_nanos(9));

    let limits = SimulationLimits::default().with_max_duties(nonzero_usize(1));
    let Err(error) = Simulation::new(
        TimelineId::new(93),
        World(Behavior::Waiting),
        topology([1, 2]),
        limits,
    ) else {
        panic!("oversized topology must reject");
    };
    assert_eq!(
        error,
        SimulationBuildError::DutyCapacity {
            limit: nonzero_usize(1),
            actual: 2,
        }
    );
    assert_eq!(
        error.to_string(),
        "simulation topology has 2 duties but limit is 1"
    );
}

#[test]
fn kernel_diagnostics_preserve_event_and_owner_identities() {
    let token = event_token();
    let cases = [
        (
            KernelFailure::UnknownDeliveryTarget {
                token,
                target: DutyId::new(9),
            },
            "event 0 targets unknown duty 9",
        ),
        (
            KernelFailure::DeliveryTargetsStopped {
                token,
                target: DutyId::new(9),
            },
            "event 0 targets stopped duty 9",
        ),
        (
            KernelFailure::DeliveryTargetMismatch {
                token,
                expected: DutyId::new(1),
                actual: DutyId::new(2),
            },
            "event 0 selected for duty 1 but owns duty 2",
        ),
        (
            KernelFailure::TimelineLostEvent(token),
            "event 0 disappeared before delivery",
        ),
        (
            KernelFailure::StoppedWithPending {
                duty: DutyId::new(1),
                pending: 3,
            },
            "duty 1 stopped with 3 pending deliveries",
        ),
        (
            KernelFailure::ActionIdsExhausted,
            "action identities are exhausted",
        ),
        (
            KernelFailure::NoFutureProgress {
                now: Moment::from_nanos(5),
            },
            "no ready action or future moment exists at 5ns",
        ),
    ];
    for (failure, expected) in cases {
        assert_eq!(failure.to_string(), expected);
    }

    assert_eq!(
        LimitFailure::ActionsAtMoment {
            at: Moment::from_nanos(4),
            limit: nonzero_u64(3),
        }
        .to_string(),
        "action limit of 3 was reached at 4ns"
    );
    assert_eq!(
        LimitFailure::VirtualTime {
            next: Moment::from_nanos(6),
            limit: Moment::from_nanos(5),
        }
        .to_string(),
        "next moment 6ns exceeds virtual-time limit 5ns"
    );
}

fn simulation(behavior: Behavior, limits: SimulationLimits) -> Simulation<World> {
    Simulation::new(TimelineId::new(93), World(behavior), topology([1]), limits)
        .unwrap_or_else(|error| panic!("simulation must build: {error}"))
}

fn event_token() -> calandria_sim::EventToken {
    let mut timeline = Timeline::new(TimelineId::new(94), TimelineLimits::default());
    timeline
        .schedule_at(Moment::from_nanos(7), ())
        .unwrap_or_else(|error| panic!("diagnostic token must fit: {error}"))
}

fn topology<const N: usize>(duties: [u32; N]) -> Topology {
    Topology::new(duties.map(DutyId::new))
        .unwrap_or_else(|error| panic!("topology must be valid: {error}"))
}

fn nonzero_usize(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap_or_else(|| panic!("test limit must be nonzero"))
}

fn nonzero_u64(value: u64) -> NonZeroU64 {
    NonZeroU64::new(value).unwrap_or_else(|| panic!("test limit must be nonzero"))
}
