//! Subscription failures at the explicit shutdown boundary.

use std::{error::Error, fmt};

/// Why a shutdown observer could not be admitted.
#[non_exhaustive]
#[derive(Debug)]
pub enum ShutdownSubscribeError<E> {
    /// The configured subscriber count is already retained.
    Full,
    /// The unique terminal owner closed before successful shutdown.
    Closed,
    /// Publishing the first shutdown request failed.
    Request(E),
}

impl<E> fmt::Display for ShutdownSubscribeError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Full => formatter.write_str("shutdown subscriber capacity is full"),
            Self::Closed => formatter.write_str("the shutdown barrier is closed"),
            Self::Request(_) => formatter.write_str("the first shutdown request failed"),
        }
    }
}

impl<E> Error for ShutdownSubscribeError<E>
where
    E: Error + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Request(source) => Some(source),
            Self::Full | Self::Closed => None,
        }
    }
}
