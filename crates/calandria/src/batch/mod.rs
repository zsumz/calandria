//! Count- and retained-byte-bounded owner-local event batches.

mod error;
mod limits;
mod owner;
mod snapshot;

pub use error::{EventBatchError, EventBatchFailure};
pub use limits::EventBatchLimits;
pub use owner::{EventBatch, EventBatchDrain};
pub use snapshot::EventBatchSnapshot;
