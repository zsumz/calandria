//! Ownership-preserving failures from one transactional model action.

mod cancellation;
mod observation;
mod send;

pub use cancellation::CancelFailure;
pub use observation::{ObservationError, ObservationFailure};
pub use send::{SendError, SendFailure};
