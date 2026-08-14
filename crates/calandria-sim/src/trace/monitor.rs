//! Monitor composition that retains a bounded causal action trace.

use calandria::{EventBatch, EventBatchFailure};

use crate::{ActionRecord, Model, Monitor, NoopMonitor, Replay, SimulationView};

use super::{TraceEntry, TraceError, TraceLimits, TraceSnapshot};

/// Bounded causal trace composed around an optional invariant monitor.
#[derive(Debug)]
pub struct Trace<N = NoopMonitor> {
    limits: TraceLimits,
    entries: EventBatch<TraceEntry>,
    monitor: N,
}

impl Trace<NoopMonitor> {
    /// Creates a trace with no additional invariant monitor.
    pub fn new(limits: TraceLimits) -> Self {
        Self::with_monitor(limits, NoopMonitor::new())
    }
}

impl<N> Trace<N> {
    /// Creates a trace that records before invoking the composed monitor.
    pub fn with_monitor(limits: TraceLimits, monitor: N) -> Self {
        Self {
            limits,
            entries: EventBatch::new(limits.batch()),
            monitor,
        }
    }

    /// Returns configured hard limits.
    pub const fn limits(&self) -> TraceLimits {
        self.limits
    }

    /// Returns retained entry and variable-byte observations.
    pub fn snapshot(&self) -> TraceSnapshot {
        TraceSnapshot::from_batch(self.limits, self.entries.snapshot())
    }

    /// Borrows causal entries in commit order.
    pub fn entries(&self) -> impl ExactSizeIterator<Item = &TraceEntry> + DoubleEndedIterator {
        self.entries.iter()
    }

    /// Returns the composed monitor.
    pub const fn monitor(&self) -> &N {
        &self.monitor
    }

    /// Builds an exact replay scheduler from the retained prefix.
    pub fn replay(&self) -> Replay {
        Replay::from_entries(self.entries.iter().copied())
    }

    /// Consumes the trace and returns its composed monitor.
    pub fn into_monitor(self) -> N {
        self.monitor
    }
}

impl<M, N> Monitor<M> for Trace<N>
where
    M: Model,
    N: Monitor<M>,
{
    type Error = TraceError<N::Error>;

    fn after_action(
        &mut self,
        view: SimulationView<'_, M>,
        action: &ActionRecord<M::Observation>,
    ) -> Result<(), Self::Error> {
        self.entries
            .try_push(TraceEntry::from_record(action))
            .map_err(|error| match error.failure() {
                EventBatchFailure::EventCapacity { limit } => TraceError::Capacity { limit },
                EventBatchFailure::RetainedByteOverflow { .. }
                | EventBatchFailure::RetainedByteCapacity { .. } => {
                    panic!("fixed-width trace entry retained variable bytes")
                }
            })?;
        self.monitor
            .after_action(view, action)
            .map_err(TraceError::Monitor)
    }
}
