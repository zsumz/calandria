//! Finite zero-or-more outcomes for one exact scripted request.

use alloc::{boxed::Box, vec::Vec};

use crate::Planned;

/// Finite outcomes produced by one scripted capability request.
///
/// An empty plan models a dropped request. Multiple outcomes model duplication,
/// multipart completion, or intentionally reordered delivery through delays.
/// Construction normalizes spare `Vec` capacity into an exact boxed slice.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Plan<T> {
    outcomes: Box<[Planned<T>]>,
}

impl<T> Plan<T> {
    /// Creates a plan from owned delayed outcomes.
    pub fn new(outcomes: Vec<Planned<T>>) -> Self {
        Self {
            outcomes: outcomes.into_boxed_slice(),
        }
    }

    /// Creates a dropped-outcome plan.
    pub fn empty() -> Self {
        Self::default()
    }

    /// Creates a plan containing exactly one delayed outcome.
    pub fn single(outcome: Planned<T>) -> Self {
        Self {
            outcomes: Box::new([outcome]),
        }
    }

    /// Returns the finite outcome count.
    pub const fn len(&self) -> usize {
        self.outcomes.len()
    }

    /// Returns whether the request produces no modeled outcome.
    pub const fn is_empty(&self) -> bool {
        self.outcomes.is_empty()
    }

    /// Borrows delayed outcomes in script order.
    pub const fn outcomes(&self) -> &[Planned<T>] {
        &self.outcomes
    }

    /// Consumes the plan and returns delayed outcomes.
    pub fn into_outcomes(self) -> Vec<Planned<T>> {
        self.outcomes.into_vec()
    }
}

impl<T> FromIterator<Planned<T>> for Plan<T> {
    fn from_iter<I: IntoIterator<Item = Planned<T>>>(iter: I) -> Self {
        Self::new(iter.into_iter().collect())
    }
}

impl<T> Default for Plan<T> {
    fn default() -> Self {
        Self {
            outcomes: Box::default(),
        }
    }
}
