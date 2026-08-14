//! One exact expected request and its finite response plan.

use super::Plan;

/// One exact scripted request and its owned response plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScriptStep<Q, R> {
    expected: Q,
    response: Plan<R>,
}

impl<Q, R> ScriptStep<Q, R> {
    /// Creates one exact script step.
    pub const fn new(expected: Q, response: Plan<R>) -> Self {
        Self { expected, response }
    }

    /// Borrows the exact expected request.
    pub const fn expected(&self) -> &Q {
        &self.expected
    }

    /// Borrows the finite response plan.
    pub const fn response(&self) -> &Plan<R> {
        &self.response
    }

    pub(super) fn into_parts(self) -> (Q, Plan<R>) {
        (self.expected, self.response)
    }
}
