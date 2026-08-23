//! Bounded shared subscription to one explicit shutdown lifecycle.

mod completer;
mod error;
mod factory;
mod requester;
mod shared;

pub use completer::ShutdownCompleter;
pub use error::ShutdownSubscribeError;
pub use factory::shutdown_barrier;
pub use requester::ShutdownRequester;
