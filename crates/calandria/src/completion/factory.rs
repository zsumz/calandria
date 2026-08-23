//! Construction of one exactly-once completion ownership pair.

use std::{cell::Cell, marker::PhantomData};

use crate::RetainedBytes;

use super::{Completer, Completion, shared};

/// Creates one completion observer and its unique terminal producer.
///
/// The observer may block, poll as a [`Future`](std::future::Future), or attempt
/// nonblocking extraction. The producer publishes one value or closes without
/// a value. Neither side is cloneable.
pub fn completion<T>() -> (Completion<T>, Completer<T>) {
    let shared = shared::Shared::new();
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

/// Returns the shared-state payload retained by one completion pair.
///
/// The payload is counted once per pair. It excludes the [`Completion`] and
/// [`Completer`] handle values, allocator bookkeeping, fixed reference-count
/// metadata, and heap memory retained indirectly by `T`.
pub fn completion_retained_bytes<T>() -> RetainedBytes {
    shared::retained_bytes::<T>()
}
