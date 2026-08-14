//! Finite delayed outcomes for deterministic capability scripts.

use calandria::Span;

/// One owned outcome planned after a relative virtual-time delay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Planned<T> {
    delay: Span,
    outcome: T,
}

impl<T> Planned<T> {
    /// Creates a delayed deterministic outcome.
    pub const fn new(delay: Span, outcome: T) -> Self {
        Self { delay, outcome }
    }

    /// Returns the delay before the outcome becomes observable.
    pub const fn delay(&self) -> Span {
        self.delay
    }

    /// Borrows the planned outcome.
    pub const fn outcome(&self) -> &T {
        &self.outcome
    }

    /// Consumes the plan and returns its outcome.
    pub fn into_outcome(self) -> T {
        self.outcome
    }

    /// Transforms the outcome while preserving its virtual delay.
    pub fn map<U>(self, map: impl FnOnce(T) -> U) -> Planned<U> {
        Planned::new(self.delay, map(self.outcome))
    }
}
