//! Exact finite capability script limit and matching tests.

use core::num::NonZeroUsize;

use calandria::{Retained, RetainedBytes, Span};
use calandria_sim::{
    ExactScript, Plan, Planned, ScriptBuildFailure, ScriptFailure, ScriptLimits, ScriptStep,
};

#[derive(Clone, Debug, Eq, PartialEq)]
struct Value(Vec<u8>);

impl Retained for Value {
    fn retained_bytes(&self) -> RetainedBytes {
        RetainedBytes::try_from(self.0.capacity())
            .unwrap_or_else(|_| panic!("test capacity must fit"))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Measured {
    id: u8,
    bytes: RetainedBytes,
}

impl Retained for Measured {
    fn retained_bytes(&self) -> RetainedBytes {
        self.bytes
    }
}

#[test]
fn mismatch_is_non_consuming_and_exhaustion_is_explicit() {
    let limits = ScriptLimits::new(nonzero(2), nonzero(2), RetainedBytes::new(32));
    let steps = vec![ScriptStep::new(
        Value(vec![1]),
        Plan::single(Planned::new(Span::from_nanos(3), Value(vec![2]))),
    )];
    let mut script = ExactScript::try_new(limits, steps)
        .unwrap_or_else(|error| panic!("script must fit: {error}"));

    assert_eq!(
        script.respond(&Value(vec![9])),
        Err(ScriptFailure::Mismatch)
    );
    assert_eq!(script.len(), 1);
    let response = script
        .respond(&Value(vec![1]))
        .unwrap_or_else(|error| panic!("exact request must match: {error}"));
    assert_eq!(response.len(), 1);
    assert!(script.is_empty());
    assert_eq!(
        script.respond(&Value(vec![1])),
        Err(ScriptFailure::Exhausted)
    );
}

#[test]
fn rejected_construction_returns_every_step() {
    let limits = ScriptLimits::new(nonzero(1), nonzero(2), RetainedBytes::new(64));
    let steps = vec![
        ScriptStep::new(Value(vec![1]), Plan::empty()),
        ScriptStep::new(Value(vec![2]), Plan::empty()),
    ];
    let Err(error) = ExactScript::<Value, Value>::try_new(limits, steps) else {
        panic!("script construction must fail");
    };
    assert!(matches!(
        error.failure(),
        ScriptBuildFailure::StepCapacity { actual: 2, .. }
    ));
    assert_eq!(error.to_string(), "script has 2 steps but limit is 1");
    assert_eq!(error.into_steps().len(), 2);
}

#[test]
fn successful_response_releases_exact_retained_bytes() {
    let limits = ScriptLimits::new(nonzero(1), nonzero(2), RetainedBytes::new(64));
    let steps = vec![ScriptStep::new(
        Value(vec![1, 2, 3]),
        Plan::single(Planned::new(Span::ZERO, Value(vec![4, 5]))),
    )];
    let mut script = ExactScript::try_new(limits, steps)
        .unwrap_or_else(|error| panic!("script must fit: {error}"));
    assert_eq!(script.limits(), limits);
    assert_eq!(script.limits().steps(), nonzero(1));
    assert_eq!(script.limits().outcomes(), nonzero(2));
    assert_eq!(script.limits().retained_bytes(), RetainedBytes::new(64));
    assert_eq!(script.expected(), Some(&Value(vec![1, 2, 3])));
    assert!(script.retained_bytes() > RetainedBytes::ZERO);

    let _ = script
        .respond(&Value(vec![1, 2, 3]))
        .unwrap_or_else(|error| panic!("request must match: {error}"));
    assert_eq!(script.retained_bytes(), RetainedBytes::ZERO);
    assert_eq!(script.expected(), None);
}

#[test]
fn zero_byte_outcomes_are_still_count_bounded() {
    let limits = ScriptLimits::new(nonzero(1), nonzero(1), RetainedBytes::ZERO);
    let steps = vec![ScriptStep::new(
        Value(Vec::new()),
        Plan::new(vec![
            Planned::new(Span::ZERO, Value(Vec::new())),
            Planned::new(Span::ZERO, Value(Vec::new())),
        ]),
    )];
    let Err(error) = ExactScript::<Value, Value>::try_new(limits, steps) else {
        panic!("script construction must fail");
    };
    assert!(matches!(
        error.failure(),
        ScriptBuildFailure::OutcomeCapacity { actual: 2, .. }
    ));
}

#[test]
fn retained_byte_rejections_distinguish_overflow_from_capacity() {
    let limits = ScriptLimits::new(nonzero(1), nonzero(1), RetainedBytes::new(u64::MAX));
    let overflow = vec![ScriptStep::new(
        measured(1, u64::MAX),
        Plan::single(Planned::new(Span::ZERO, measured(2, 1))),
    )];
    let Err(error) = ExactScript::try_new(limits, overflow) else {
        panic!("retained-byte accounting must not wrap");
    };
    assert_eq!(error.failure(), ScriptBuildFailure::RetainedByteOverflow);
    assert_eq!(error.into_steps().len(), 1);

    let capacity = vec![ScriptStep::new(
        measured(3, 2),
        Plan::single(Planned::new(Span::ZERO, measured(4, 3))),
    )];
    let limits = ScriptLimits::new(nonzero(1), nonzero(1), RetainedBytes::new(4));
    let Err(error) = ExactScript::try_new(limits, capacity) else {
        panic!("script above its retained-byte limit must reject");
    };
    assert_eq!(
        error.failure(),
        ScriptBuildFailure::RetainedByteCapacity {
            limit: RetainedBytes::new(4),
            actual: RetainedBytes::new(5),
        }
    );
    assert_eq!(error.into_steps().len(), 1);
}

const fn measured(id: u8, bytes: u64) -> Measured {
    Measured {
        id,
        bytes: RetainedBytes::new(bytes),
    }
}

fn nonzero(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap_or_else(|| panic!("test limit must be nonzero"))
}
