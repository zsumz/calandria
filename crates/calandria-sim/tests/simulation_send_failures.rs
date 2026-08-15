//! Transactional modeled-send rejection and ownership tests.

use calandria::{Moment, RetainedBytes, Span};
use calandria_sim::{DutyId, ScheduleFailure, SendFailure, SimulationLimits, TimelineLimits};

#[path = "simulation_send_support/mod.rs"]
mod support;

use support::{
    Scenario, assert_send_failure, event, inject_future, nonzero, simulation, step, topology,
};

#[test]
fn action_effect_limits_reject_exact_owned_events() {
    let limits = SimulationLimits::default().with_effects_per_action(nonzero(1));
    assert_send_failure(
        Scenario::EffectCapacity,
        limits,
        Moment::ORIGIN,
        topology([1]),
        event(2, 0),
        SendFailure::EffectCapacity { limit: nonzero(1) },
    );

    let limits = SimulationLimits::default().with_effect_bytes_per_action(RetainedBytes::new(3));
    assert_send_failure(
        Scenario::EffectByteCapacity,
        limits,
        Moment::ORIGIN,
        topology([1]),
        event(2, 2),
        SendFailure::RetainedByteCapacity {
            limit: RetainedBytes::new(3),
            current: RetainedBytes::new(2),
            event: RetainedBytes::new(2),
        },
    );

    let limits =
        SimulationLimits::default().with_effect_bytes_per_action(RetainedBytes::new(u64::MAX));
    assert_send_failure(
        Scenario::EffectByteOverflow,
        limits,
        Moment::ORIGIN,
        topology([1]),
        event(2, u64::MAX),
        SendFailure::RetainedByteOverflow {
            current: RetainedBytes::new(1),
            event: RetainedBytes::new(u64::MAX),
        },
    );
}

#[test]
fn target_and_time_validation_precede_timeline_admission() {
    assert_send_failure(
        Scenario::UnknownTarget,
        SimulationLimits::default(),
        Moment::ORIGIN,
        topology([1]),
        event(3, 0),
        SendFailure::UnknownTarget(DutyId::new(99)),
    );

    let mut stopped = simulation(
        Scenario::TargetStopped,
        SimulationLimits::default(),
        Moment::ORIGIN,
        topology([1, 2]),
    );
    step(&mut stopped);
    step(&mut stopped);
    assert_eq!(
        stopped.model().rejected,
        Some((event(4, 0), SendFailure::TargetStopped(DutyId::new(1))))
    );

    assert_send_failure(
        Scenario::TimeOverflow,
        SimulationLimits::default(),
        Moment::from_nanos(u64::MAX),
        topology([1]),
        event(5, 0),
        SendFailure::TimeOverflow {
            current: Moment::from_nanos(u64::MAX),
            delay: Span::from_nanos(1),
        },
    );

    let limits = SimulationLimits::default().with_max_virtual_time(Moment::from_nanos(5));
    assert_send_failure(
        Scenario::BeyondTime,
        limits,
        Moment::ORIGIN,
        topology([1]),
        event(6, 0),
        SendFailure::BeyondTimeLimit {
            requested: Moment::from_nanos(6),
            limit: Moment::from_nanos(5),
        },
    );
}

#[test]
fn timeline_rejections_are_preserved_at_the_action_boundary() {
    let count_limits =
        SimulationLimits::new(TimelineLimits::new(nonzero(1), RetainedBytes::new(8)));
    let mut count = simulation(
        Scenario::TimelineCount,
        count_limits,
        Moment::ORIGIN,
        topology([1]),
    );
    inject_future(&mut count, event(20, 1));
    step(&mut count);
    assert_eq!(
        count.model().rejected,
        Some((
            event(7, 0),
            SendFailure::Timeline(ScheduleFailure::EventCapacity { limit: nonzero(1) })
        ))
    );

    let byte_limits = SimulationLimits::new(TimelineLimits::new(nonzero(2), RetainedBytes::new(3)));
    let mut bytes = simulation(
        Scenario::TimelineBytes,
        byte_limits,
        Moment::ORIGIN,
        topology([1]),
    );
    inject_future(&mut bytes, event(21, 2));
    step(&mut bytes);
    assert_eq!(
        bytes.model().rejected,
        Some((
            event(8, 2),
            SendFailure::Timeline(ScheduleFailure::RetainedByteCapacity {
                limit: RetainedBytes::new(3),
                current: RetainedBytes::new(2),
                event: RetainedBytes::new(2),
            })
        ))
    );

    let overflow_limits = SimulationLimits::new(TimelineLimits::new(
        nonzero(2),
        RetainedBytes::new(u64::MAX),
    ))
    .with_effect_bytes_per_action(RetainedBytes::new(u64::MAX));
    let mut overflow = simulation(
        Scenario::TimelineOverflow,
        overflow_limits,
        Moment::ORIGIN,
        topology([1]),
    );
    inject_future(&mut overflow, event(22, 1));
    step(&mut overflow);
    assert_eq!(
        overflow.model().rejected,
        Some((
            event(9, u64::MAX),
            SendFailure::Timeline(ScheduleFailure::RetainedByteOverflow {
                current: RetainedBytes::new(1),
                event: RetainedBytes::new(u64::MAX),
            })
        ))
    );

    assert_send_failure(
        Scenario::TimelinePast,
        SimulationLimits::default(),
        Moment::from_nanos(10),
        topology([1]),
        event(10, 0),
        SendFailure::Timeline(ScheduleFailure::ScheduledInPast {
            current: Moment::from_nanos(10),
            requested: Moment::from_nanos(9),
        }),
    );
}
