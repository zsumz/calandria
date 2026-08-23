//! Exactly-once runtime-neutral terminal observation.

mod error;
mod factory;
mod observer;
mod producer;
mod shared;

pub use error::CompletionError;
pub use factory::completion;
pub use observer::Completion;
pub use producer::Completer;
