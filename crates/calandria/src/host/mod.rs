//! Bounded execution for reactors and other explicit single-owner duties.

mod action;
mod clock;
mod config;
mod duty;
mod embedded;
mod error;
mod snapshot;
#[cfg(feature = "std")]
mod waiter;

pub use action::{HostAction, HostStep};
pub use clock::Clock;
#[cfg(feature = "std")]
pub use clock::MonotonicClock;
pub use config::HostConfig;
pub use duty::Duty;
pub use embedded::EmbeddedHost;
pub use error::HostError;
pub use snapshot::{HostPhase, HostSnapshot};
#[cfg(feature = "std")]
pub use waiter::{ThreadNotifier, ThreadParker, WaitOutcome, Waiter, thread_parker};
