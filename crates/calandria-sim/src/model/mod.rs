//! Typed adaptation of concrete owner semantics to deterministic execution.

mod context;
mod contract;
mod effect;
mod identity;
mod routed;
mod topology;

pub use context::ActionContext;
pub(crate) use context::ActionContextLimits;
pub use contract::Model;
pub use effect::{
    CancelFailure, ObservationError, ObservationFailure, SendError, SendFailure,
};
pub use identity::DutyId;
pub(crate) use routed::Routed;
pub use topology::{Topology, TopologyError};
