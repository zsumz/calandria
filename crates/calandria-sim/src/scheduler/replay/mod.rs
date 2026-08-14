//! Exact committed-action replay and divergence diagnostics.

mod error;
mod owner;

pub use error::{ReplayDivergence, ReplayPosition};
pub use owner::Replay;
