//! Ownership-preserving script construction and request failures.

use alloc::vec::Vec;
use core::{fmt, num::NonZeroUsize};

use calandria::RetainedBytes;

use super::ScriptStep;

/// Why an exact finite script could not retain its supplied steps.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScriptBuildFailure {
    /// The script contains more steps than its hard count limit.
    StepCapacity { limit: NonZeroUsize, actual: usize },
    /// The script contains more response outcomes than its hard count limit.
    OutcomeCapacity { limit: NonZeroUsize, actual: usize },
    /// Response-outcome count accounting overflowed.
    OutcomeCountOverflow,
    /// Variable retained-byte accounting overflowed.
    RetainedByteOverflow,
    /// The script exceeds its variable retained-byte limit.
    RetainedByteCapacity {
        limit: RetainedBytes,
        actual: RetainedBytes,
    },
}

impl fmt::Display for ScriptBuildFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StepCapacity { limit, actual } => {
                write!(formatter, "script has {actual} steps but limit is {limit}")
            }
            Self::OutcomeCapacity { limit, actual } => {
                write!(formatter, "script has {actual} outcomes but limit is {limit}")
            }
            Self::OutcomeCountOverflow => {
                formatter.write_str("script outcome-count accounting overflowed")
            }
            Self::RetainedByteOverflow => {
                formatter.write_str("script retained-byte accounting overflowed")
            }
            Self::RetainedByteCapacity { limit, actual } => write!(
                formatter,
                "script retains {} variable bytes but limit is {}",
                actual.get(),
                limit.get()
            ),
        }
    }
}

impl core::error::Error for ScriptBuildFailure {}

/// Construction failure retaining ownership of every supplied script step.
#[derive(Debug)]
pub struct ScriptBuildError<Q, R> {
    steps: Vec<ScriptStep<Q, R>>,
    failure: ScriptBuildFailure,
}

impl<Q, R> ScriptBuildError<Q, R> {
    pub(super) const fn new(
        steps: Vec<ScriptStep<Q, R>>,
        failure: ScriptBuildFailure,
    ) -> Self {
        Self { steps, failure }
    }

    /// Returns the construction failure.
    pub const fn failure(&self) -> ScriptBuildFailure {
        self.failure
    }

    /// Returns ownership of every rejected step in original order.
    pub fn into_steps(self) -> Vec<ScriptStep<Q, R>> {
        self.steps
    }
}

impl<Q, R> fmt::Display for ScriptBuildError<Q, R> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.failure.fmt(formatter)
    }
}

impl<Q: fmt::Debug, R: fmt::Debug> core::error::Error for ScriptBuildError<Q, R> {}

/// Why an exact scripted request did not produce a response plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScriptFailure {
    /// No scripted request remains.
    Exhausted,
    /// The request did not match the next exact expected value.
    Mismatch,
}

impl fmt::Display for ScriptFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Exhausted => formatter.write_str("script is exhausted"),
            Self::Mismatch => formatter.write_str("request does not match the next script step"),
        }
    }
}

impl core::error::Error for ScriptFailure {}
