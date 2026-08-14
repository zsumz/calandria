//! Bounded shared subscription to one explicit shutdown lifecycle.

mod completer;
mod error;
mod requester;
mod shared;

use std::{num::NonZeroUsize, sync::Arc};

pub use completer::ShutdownCompleter;
pub use error::ShutdownSubscribeError;
pub use requester::ShutdownRequester;

use shared::Shared;

/// Creates one bounded shutdown subscription authority and terminal owner.
///
/// The requester is cloneable. The completer is unique and publishes successful
/// shutdown to every admitted observer. Explicitly closing or dropping the
/// unsettled completer closes the barrier and every admitted completion.
pub fn shutdown_barrier(capacity: NonZeroUsize) -> (ShutdownRequester, ShutdownCompleter) {
    let shared = Arc::new(Shared::new(capacity));
    (
        ShutdownRequester {
            shared: Arc::clone(&shared),
        },
        ShutdownCompleter {
            shared,
            settled: false,
        },
    )
}
