//! Hard retained topology limits for one reactor group.

use std::num::NonZeroUsize;

/// Hard count limit for a static reactor group.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReactorGroupLimits {
    reactors: NonZeroUsize,
}

impl ReactorGroupLimits {
    /// Creates a group limit with an explicit maximum reactor count.
    pub const fn new(reactors: NonZeroUsize) -> Self {
        Self { reactors }
    }

    /// Returns the maximum retained reactor count.
    pub const fn reactors(self) -> NonZeroUsize {
        self.reactors
    }
}
