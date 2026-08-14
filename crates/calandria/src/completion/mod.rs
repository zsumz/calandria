//! Exactly-once runtime-neutral terminal observation.

mod error;
mod observer;
mod producer;
mod shared;

use std::{cell::Cell, marker::PhantomData};

pub use error::CompletionError;
pub use observer::Completion;
pub use producer::Completer;

use shared::Shared;

/// Creates one completion observer and its unique terminal producer.
///
/// The observer may block, poll as a [`Future`], or
/// attempt nonblocking extraction. The producer publishes one value or closes
/// without a value. Neither side is cloneable.
pub fn completion<T>() -> (Completion<T>, Completer<T>) {
    let shared = Shared::new();
    (
        Completion {
            shared: shared.clone(),
            _single_observer: PhantomData::<Cell<()>>,
        },
        Completer {
            shared,
            settled: false,
        },
    )
}
