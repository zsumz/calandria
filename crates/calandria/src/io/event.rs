//! Readiness observations separated from administrative wakes.

use crate::ResourceToken;

use super::Readiness;

/// One external progress observation returned by a poller.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PollEvent {
    /// Administrative or cross-thread work requested another owner turn.
    Wake,
    /// One generation-checked resource may make nonblocking progress.
    Resource {
        /// Generational token identifying the registered resource.
        token: ResourceToken,
        /// Readiness retained without imposing resource policy.
        readiness: Readiness,
    },
}

impl PollEvent {
    /// Returns the resource token when this is resource readiness.
    pub const fn resource(self) -> Option<ResourceToken> {
        match self {
            Self::Wake => None,
            Self::Resource { token, .. } => Some(token),
        }
    }

    /// Returns readiness when this is a resource event.
    pub const fn readiness(self) -> Option<Readiness> {
        match self {
            Self::Wake => None,
            Self::Resource { readiness, .. } => Some(readiness),
        }
    }
}
