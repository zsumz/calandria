//! Bounded virtual-time event ownership with stable same-time delivery.

mod error;
mod limits;
mod owner;
mod snapshot;
mod transaction;

pub use error::{ScheduleError, ScheduleFailure};
pub use limits::TimelineLimits;
pub use owner::Timeline;
pub use snapshot::TimelineSnapshot;
