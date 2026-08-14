//! Mio readiness for bounded Calandria reactor duties.
//!
//! This adapter owns operating-system registration, polling, and wake
//! translation. Protocol state, resource lifecycle, retry, and shutdown policy
//! remain in the concrete reactor.

#![forbid(unsafe_code)]

pub mod error;
pub mod limits;
pub mod poller;
mod registrations;
pub mod snapshot;
mod translation;

pub use error::MioError;
pub use limits::MioPollerLimits;
pub use poller::MioPoller;
pub use snapshot::MioPollerSnapshot;
