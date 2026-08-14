//! Post-commit invariant observation without execution authority.

use core::convert::Infallible;

use crate::{ActionRecord, Model};

use super::SimulationView;

/// Checks immutable model and kernel state after every committed action.
pub trait Monitor<M: Model> {
    /// Monitor-defined invariant failure.
    type Error;

    /// Observes one committed action and its resulting world state.
    fn after_action(
        &mut self,
        view: SimulationView<'_, M>,
        action: &ActionRecord<M::Observation>,
    ) -> Result<(), Self::Error>;
}

/// Monitor that accepts every committed state.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoopMonitor;

impl NoopMonitor {
    /// Creates a monitor with no consumer invariants.
    pub const fn new() -> Self {
        Self
    }
}

impl<M: Model> Monitor<M> for NoopMonitor {
    type Error = Infallible;

    fn after_action(
        &mut self,
        _view: SimulationView<'_, M>,
        _action: &ActionRecord<M::Observation>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}
