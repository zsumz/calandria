//! Portable bounded action selection from one explicit entropy stream.

use core::fmt;

use calandria::Moment;

use crate::{ActionKey, EntropySeed, EntropyStreamId, ReadySet, Scheduler, SplitMix64};

/// Bounded deterministic scheduler driven by `SplitMix64` version 1.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Seeded {
    entropy: SplitMix64,
    selections: u64,
}

impl Seeded {
    /// Public version of the bounded selection mapping.
    pub const ALGORITHM_VERSION: u16 = 1;

    /// Creates a scheduler from an explicit root seed and stream namespace.
    pub const fn new(seed: EntropySeed, stream: EntropyStreamId) -> Self {
        Self {
            entropy: SplitMix64::new(seed, stream),
            selections: 0,
        }
    }

    /// Returns the root seed.
    pub const fn seed(self) -> EntropySeed {
        self.entropy.seed()
    }

    /// Returns the scheduler stream namespace.
    pub const fn stream(self) -> EntropyStreamId {
        self.entropy.stream()
    }

    /// Returns successful selection count.
    pub const fn selections(self) -> u64 {
        self.selections
    }
}

impl Scheduler for Seeded {
    type Error = SeededError;

    fn choose(&mut self, _now: Moment, ready: ReadySet<'_>) -> Result<ActionKey, Self::Error> {
        let Some(next_selections) = self.selections.checked_add(1) else {
            return Err(SeededError::SelectionsExhausted);
        };
        let actions = u64::try_from(ready.len()).map_err(|_| SeededError::ReadySetTooLarge {
            actions: ready.len(),
        })?;
        assert!(actions != 0, "scheduler requires a nonempty ready set");

        let draw = self.entropy.next_u64();
        let scaled = (u128::from(draw) * u128::from(actions)) >> 64;
        let index = usize::try_from(scaled)
            .unwrap_or_else(|_| panic!("scaled scheduler index exceeds local address space"));
        self.selections = next_selections;
        Ok(ready.actions()[index])
    }
}

/// Failure to perform one bounded seeded selection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeededError {
    /// The local ready set cannot be represented by the fixed-width mapping.
    ReadySetTooLarge {
        /// Number of enabled actions.
        actions: usize,
    },
    /// The fixed-width selection counter was exhausted.
    SelectionsExhausted,
}

impl fmt::Display for SeededError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadySetTooLarge { actions } => {
                write!(formatter, "ready set of {actions} actions exceeds u64")
            }
            Self::SelectionsExhausted => {
                formatter.write_str("seeded scheduler selection identities are exhausted")
            }
        }
    }
}

impl core::error::Error for SeededError {}
