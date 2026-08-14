//! Bounded exact request matching with non-consuming mismatch.

use alloc::{collections::VecDeque, vec::Vec};

use calandria::{Retained, RetainedBytes};

use super::{
    Plan, ScriptBuildError, ScriptBuildFailure, ScriptFailure, ScriptLimits,
    ScriptStep,
};

/// Count- and byte-bounded exact finite capability script.
#[derive(Debug)]
pub struct ExactScript<Q, R> {
    limits: ScriptLimits,
    retained: RetainedBytes,
    steps: VecDeque<MeasuredStep<Q, R>>,
}

impl<Q: Retained, R: Retained> ExactScript<Q, R> {
    /// Retains all supplied steps or returns them unchanged on rejection.
    pub fn try_new(
        limits: ScriptLimits,
        steps: Vec<ScriptStep<Q, R>>,
    ) -> Result<Self, ScriptBuildError<Q, R>> {
        if steps.len() > limits.steps().get() {
            let actual = steps.len();
            return Err(ScriptBuildError::new(
                steps,
                ScriptBuildFailure::StepCapacity {
                    limit: limits.steps(),
                    actual,
                },
            ));
        }
        let outcomes = match count_outcomes(&steps) {
            Ok(outcomes) => outcomes,
            Err(failure) => return Err(ScriptBuildError::new(steps, failure)),
        };
        if outcomes > limits.outcomes().get() {
            return Err(ScriptBuildError::new(
                steps,
                ScriptBuildFailure::OutcomeCapacity {
                    limit: limits.outcomes(),
                    actual: outcomes,
                },
            ));
        }
        let (retained, measurements) = match measure_steps(&steps) {
            Ok(measured) => measured,
            Err(failure) => return Err(ScriptBuildError::new(steps, failure)),
        };
        if retained > limits.retained_bytes() {
            return Err(ScriptBuildError::new(
                steps,
                ScriptBuildFailure::RetainedByteCapacity {
                    limit: limits.retained_bytes(),
                    actual: retained,
                },
            ));
        }
        let measured = steps
            .into_iter()
            .zip(measurements)
            .map(|(step, retained)| MeasuredStep { step, retained })
            .collect();
        Ok(Self {
            limits,
            retained,
            steps: measured,
        })
    }

    /// Returns configured hard ownership limits.
    pub const fn limits(&self) -> ScriptLimits {
        self.limits
    }

    /// Returns the remaining exact request count.
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    /// Returns whether no scripted request remains.
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// Returns variable bytes retained by remaining requests and responses.
    pub const fn retained_bytes(&self) -> RetainedBytes {
        self.retained
    }

    /// Borrows the next exact expected request without consuming it.
    pub fn expected(&self) -> Option<&Q> {
        self.steps.front().map(|measured| measured.step.expected())
    }
}

impl<Q: Retained + PartialEq, R: Retained> ExactScript<Q, R> {
    /// Consumes the next response only when the request matches exactly.
    pub fn respond(&mut self, request: &Q) -> Result<Plan<R>, ScriptFailure> {
        let Some(next) = self.steps.front() else {
            return Err(ScriptFailure::Exhausted);
        };
        if next.step.expected() != request {
            return Err(ScriptFailure::Mismatch);
        }
        let measured = self
            .steps
            .pop_front()
            .unwrap_or_else(|| panic!("matched script step must remain present"));
        self.retained = self
            .retained
            .checked_sub(measured.retained)
            .unwrap_or_else(|| panic!("script retained-byte accounting must be exact"));
        let (_, response) = measured.step.into_parts();
        Ok(response)
    }
}

#[derive(Debug)]
struct MeasuredStep<Q, R> {
    step: ScriptStep<Q, R>,
    retained: RetainedBytes,
}

fn count_outcomes<Q, R>(
    steps: &[ScriptStep<Q, R>],
) -> Result<usize, ScriptBuildFailure> {
    let mut total = 0usize;
    for step in steps {
        let Some(next) = total.checked_add(step.response().len()) else {
            return Err(ScriptBuildFailure::OutcomeCountOverflow);
        };
        total = next;
    }
    Ok(total)
}

fn measure_steps<Q: Retained, R: Retained>(
    steps: &[ScriptStep<Q, R>],
) -> Result<(RetainedBytes, Vec<RetainedBytes>), ScriptBuildFailure> {
    let mut total = RetainedBytes::ZERO;
    let mut measurements = Vec::with_capacity(steps.len());
    for step in steps {
        let Some(measured) = measure_step(step) else {
            return Err(ScriptBuildFailure::RetainedByteOverflow);
        };
        let Some(next) = total.checked_add(measured) else {
            return Err(ScriptBuildFailure::RetainedByteOverflow);
        };
        measurements.push(measured);
        total = next;
    }
    Ok((total, measurements))
}

fn measure_step<Q: Retained, R: Retained>(
    step: &ScriptStep<Q, R>,
) -> Option<RetainedBytes> {
    let mut total = step.expected().retained_bytes();
    for outcome in step.response().outcomes() {
        total = total.checked_add(outcome.outcome().retained_bytes())?;
    }
    Some(total)
}
