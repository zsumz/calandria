//! Public deterministic value, limit, topology, and failure contracts.

use std::error::Error;

use calandria::{Moment, RetainedBytes, Span};
use calandria_sim::{
    ActionKey, ClockError, DutyId, EntropySeed, EntropyStreamId, InjectionFailure, KernelFailure,
    LimitFailure, RunError, ScheduleFailure, ScriptBuildFailure, ScriptFailure,
    SimulationBuildFailure, SimulationPhase, StepError, Topology, TopologyError, TraceError,
    TraceLimits,
};

#[path = "public_contracts_support/mod.rs"]
mod support;

use support::{PlannedFailure, nonzero, nonzero_u64};

#[test]
fn script_failures_have_exact_capacity_overflow_and_request_diagnostics() {
    let cases = [
        (
            ScriptBuildFailure::StepCapacity {
                limit: nonzero(2),
                actual: 3,
            },
            "script has 3 steps but limit is 2",
        ),
        (
            ScriptBuildFailure::OutcomeCapacity {
                limit: nonzero(4),
                actual: 5,
            },
            "script has 5 outcomes but limit is 4",
        ),
        (
            ScriptBuildFailure::OutcomeCountOverflow,
            "script outcome-count accounting overflowed",
        ),
        (
            ScriptBuildFailure::RetainedByteOverflow,
            "script retained-byte accounting overflowed",
        ),
        (
            ScriptBuildFailure::RetainedByteCapacity {
                limit: RetainedBytes::new(6),
                actual: RetainedBytes::new(7),
            },
            "script retains 7 variable bytes but limit is 6",
        ),
    ];
    for (failure, expected) in cases {
        assert_eq!(failure.to_string(), expected);
        assert!(Error::source(&failure).is_none());
    }
    assert_eq!(ScriptFailure::Exhausted.to_string(), "script is exhausted");
    assert_eq!(
        ScriptFailure::Mismatch.to_string(),
        "request does not match the next script step"
    );
}

#[test]
fn schedule_failures_name_exact_time_count_and_byte_boundaries() {
    let cases = [
        (
            ScheduleFailure::ScheduledInPast {
                current: Moment::from_nanos(5),
                requested: Moment::from_nanos(4),
            },
            "cannot schedule at 4ns before current virtual time 5ns",
        ),
        (
            ScheduleFailure::TimeOverflow {
                current: Moment::from_nanos(u64::MAX),
                delay: Span::from_nanos(1),
            },
            "scheduling 1ns after 18446744073709551615ns would overflow virtual time",
        ),
        (
            ScheduleFailure::EventCapacity { limit: nonzero(3) },
            "pending event capacity of 3 was reached",
        ),
        (
            ScheduleFailure::RetainedByteOverflow {
                current: RetainedBytes::new(u64::MAX),
                event: RetainedBytes::new(1),
            },
            "adding 1 retained bytes to 18446744073709551615 would overflow accounting",
        ),
        (
            ScheduleFailure::RetainedByteCapacity {
                limit: RetainedBytes::new(8),
                current: RetainedBytes::new(7),
                event: RetainedBytes::new(2),
            },
            "adding 2 retained bytes to 7 would exceed limit 8",
        ),
        (
            ScheduleFailure::EventIdsExhausted,
            "event identities are exhausted",
        ),
    ];
    for (failure, expected) in cases {
        assert_eq!(failure.to_string(), expected);
        assert!(Error::source(&failure).is_none());
    }
}

#[test]
fn topology_clock_and_construction_failures_are_explicit() {
    let topology = Topology::new([DutyId::new(3), DutyId::new(1)])
        .unwrap_or_else(|error| panic!("topology must be valid: {error}"));
    assert_eq!(topology.len(), 2);
    assert!(!topology.is_empty());
    assert_eq!(topology.duties(), [DutyId::new(1), DutyId::new(3)]);
    assert!(topology.contains(DutyId::new(1)));
    assert!(!topology.contains(DutyId::new(2)));
    assert_eq!(Topology::new([]), Err(TopologyError::Empty));
    assert_eq!(
        Topology::new([DutyId::new(1), DutyId::new(1)]),
        Err(TopologyError::Duplicate(DutyId::new(1)))
    );
    assert_eq!(
        TopologyError::Empty.to_string(),
        "simulation topology must contain a duty"
    );
    assert_eq!(
        TopologyError::Duplicate(DutyId::new(9)).to_string(),
        "duty 9 appears more than once"
    );

    assert_eq!(
        ClockError::MovesBackward {
            current: Moment::from_nanos(5),
            requested: Moment::from_nanos(4),
        }
        .to_string(),
        "virtual time cannot move from 5ns back to 4ns"
    );
    assert_eq!(
        ClockError::Overflow {
            current: Moment::from_nanos(u64::MAX),
            span: Span::from_nanos(1),
        }
        .to_string(),
        "advancing virtual time from 18446744073709551615ns by 1ns would overflow"
    );

    let build = [
        (
            SimulationBuildFailure::DutyCapacity {
                limit: nonzero(2),
                actual: 3,
            },
            "simulation topology has 3 duties but limit is 2",
        ),
        (
            SimulationBuildFailure::ReadyCapacityOverflow {
                duties: usize::MAX,
                events: nonzero(1),
            },
            "combining 18446744073709551615 duties with 1 pending events overflows ready capacity",
        ),
        (
            SimulationBuildFailure::InitialTimeBeyondLimit {
                initial: Moment::from_nanos(6),
                limit: Moment::from_nanos(5),
            },
            "initial moment 6ns exceeds virtual-time limit 5ns",
        ),
    ];
    for (failure, expected) in build {
        assert_eq!(failure.to_string(), expected);
    }
}

#[test]
fn injection_step_run_and_trace_failures_keep_layered_diagnostics() {
    type ErrorType = StepError<PlannedFailure, PlannedFailure, PlannedFailure>;

    let injection = [
        (InjectionFailure::Poisoned, "simulation is poisoned"),
        (
            InjectionFailure::Inactive(SimulationPhase::Completed),
            "simulation is Completed",
        ),
        (
            InjectionFailure::UnknownTarget(DutyId::new(7)),
            "unknown duty 7",
        ),
        (
            InjectionFailure::TargetStopped(DutyId::new(8)),
            "duty 8 has stopped",
        ),
        (
            InjectionFailure::TimeOverflow {
                current: Moment::from_nanos(u64::MAX),
                delay: Span::from_nanos(1),
            },
            "scheduling 1ns after 18446744073709551615ns would overflow virtual time",
        ),
        (
            InjectionFailure::BeyondTimeLimit {
                requested: Moment::from_nanos(6),
                limit: Moment::from_nanos(5),
            },
            "event at 6ns exceeds virtual-time limit 5ns",
        ),
        (
            InjectionFailure::Timeline(ScheduleFailure::EventIdsExhausted),
            "event identities are exhausted",
        ),
    ];
    for (failure, expected) in injection {
        assert_eq!(failure.to_string(), expected);
    }

    let steps = [
        (
            ErrorType::Scheduler(PlannedFailure),
            "scheduler failed: planned failure",
        ),
        (
            ErrorType::InvalidSelection(ActionKey::turn(DutyId::new(9))),
            "scheduler selected unavailable action for duty 9",
        ),
        (
            ErrorType::Kernel(KernelFailure::ActionIdsExhausted),
            "action identities are exhausted",
        ),
        (
            ErrorType::Limit(LimitFailure::TotalActions {
                limit: nonzero_u64(4),
            }),
            "total action limit of 4 was reached",
        ),
        (ErrorType::Poisoned, "simulation is poisoned"),
        (
            ErrorType::Inactive(SimulationPhase::Failed),
            "simulation is Failed",
        ),
    ];
    for (failure, expected) in steps {
        assert_eq!(failure.to_string(), expected);
    }
    let run = RunError::Step(PlannedFailure);
    assert_eq!(run.to_string(), "planned failure");

    let capacity: TraceError<PlannedFailure> = TraceError::Capacity { limit: nonzero(3) };
    assert_eq!(
        capacity.to_string(),
        "causal trace entry limit of 3 was reached"
    );
    assert!(Error::source(&capacity).is_none());
    let monitor = TraceError::Monitor(PlannedFailure);
    assert_eq!(
        monitor.to_string(),
        "composed monitor failed: planned failure"
    );
    assert_eq!(
        Error::source(&monitor).map(ToString::to_string),
        Some(String::from("planned failure"))
    );
}

#[test]
fn fixed_identity_and_trace_limit_accessors_round_trip() {
    let seed = EntropySeed::new(17);
    let stream = EntropyStreamId::new(19);
    assert_eq!(seed.get(), 17);
    assert_eq!(stream.get(), 19);

    let limits = TraceLimits::new(nonzero(5), RetainedBytes::new(7));
    assert_eq!(limits.entries(), nonzero(5));
    assert_eq!(limits.retained_bytes(), RetainedBytes::new(7));
}
