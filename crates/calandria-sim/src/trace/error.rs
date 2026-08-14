//! Trace capacity and composed-monitor failure boundaries.

use core::{fmt, num::NonZeroUsize};

/// Failure after a committed action reached its bounded trace monitor.
#[derive(Debug)]
pub enum TraceError<MonitorError> {
    /// The trace could not retain another fixed causal entry.
    Capacity {
        /// Configured trace entry limit.
        limit: NonZeroUsize,
    },
    /// The composed consumer monitor rejected committed state.
    Monitor(MonitorError),
}

impl<E: fmt::Display> fmt::Display for TraceError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Capacity { limit } => {
                write!(formatter, "causal trace entry limit of {limit} was reached")
            }
            Self::Monitor(source) => write!(formatter, "composed monitor failed: {source}"),
        }
    }
}

impl<E> core::error::Error for TraceError<E>
where
    E: core::error::Error + 'static,
{
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Capacity { .. } => None,
            Self::Monitor(source) => Some(source),
        }
    }
}
