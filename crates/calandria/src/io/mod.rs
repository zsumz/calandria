//! Backend-neutral bounded readiness vocabulary.

mod event;
mod events;
mod interest;
mod poller;
mod readiness;

pub use event::PollEvent;
pub use events::{PollEvents, PollEventsDrain, PollEventsError};
pub use interest::Interest;
pub use poller::{PollReport, Poller};
pub use readiness::Readiness;
