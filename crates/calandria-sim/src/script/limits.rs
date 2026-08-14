//! Hard count and retained-byte limits for exact finite scripts.

use core::num::NonZeroUsize;

use calandria::RetainedBytes;

/// Hard ownership limits for one exact finite script.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScriptLimits {
    steps: NonZeroUsize,
    outcomes: NonZeroUsize,
    retained_bytes: RetainedBytes,
}

impl ScriptLimits {
    /// Creates exact script limits.
    pub const fn new(
        steps: NonZeroUsize,
        outcomes: NonZeroUsize,
        retained_bytes: RetainedBytes,
    ) -> Self {
        Self {
            steps,
            outcomes,
            retained_bytes,
        }
    }

    /// Returns the maximum retained script-step count.
    pub const fn steps(self) -> NonZeroUsize {
        self.steps
    }

    /// Returns the maximum total response-outcome count.
    pub const fn outcomes(self) -> NonZeroUsize {
        self.outcomes
    }

    /// Returns the maximum variable retained bytes.
    pub const fn retained_bytes(self) -> RetainedBytes {
        self.retained_bytes
    }
}
