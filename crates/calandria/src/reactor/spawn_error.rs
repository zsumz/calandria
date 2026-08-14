//! Ownership-preserving failure to start one reactor thread.

use std::{fmt, io};

use super::Reactor;

/// Failed reactor thread creation with the unstarted reactor returned.
pub struct ReactorSpawnError<D, C, W> {
    source: io::Error,
    reactor: Reactor<D, C, W>,
}

impl<D, C, W> ReactorSpawnError<D, C, W> {
    pub(super) const fn new(source: io::Error, reactor: Reactor<D, C, W>) -> Self {
        Self { source, reactor }
    }

    /// Returns the operating-system thread creation failure.
    pub const fn source_error(&self) -> &io::Error {
        &self.source
    }

    /// Consumes the error and returns the unstarted reactor.
    pub fn into_reactor(self) -> Reactor<D, C, W> {
        self.reactor
    }

    /// Consumes the error and returns both failure and unstarted reactor.
    pub fn into_parts(self) -> (io::Error, Reactor<D, C, W>) {
        (self.source, self.reactor)
    }
}

impl<D, C, W> fmt::Display for ReactorSpawnError<D, C, W> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "reactor thread creation failed: {}", self.source)
    }
}

impl<D, C, W> fmt::Debug for ReactorSpawnError<D, C, W> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReactorSpawnError")
            .field("source", &self.source)
            .finish_non_exhaustive()
    }
}

impl<D, C, W> std::error::Error for ReactorSpawnError<D, C, W> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}
