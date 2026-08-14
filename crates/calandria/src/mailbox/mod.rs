//! Count- and byte-bounded multi-producer, single-consumer reactor mailbox.

mod config;
mod error;
mod receiver;
mod sender;
mod shared;
mod snapshot;

use std::{
    marker::PhantomData,
    sync::{Arc, Mutex},
};

use crate::{Retained, RetainedBytes, WakeHandle};

pub use config::{Lane, LaneLimits, MailboxLimits};
pub use error::{AdmissionFailure, TrySendError};
pub use receiver::MailboxReceiver;
pub use sender::MailboxSender;
pub use snapshot::{DrainReport, DrainStatus, LaneSnapshot, MailboxSnapshot};

use shared::{Counters, Shared, State};

/// Creates a mailbox using [`Retained`] for admission accounting.
pub fn mailbox<T: Retained>(
    limits: MailboxLimits,
    wake: WakeHandle,
) -> (MailboxSender<T>, MailboxReceiver<T>) {
    mailbox_with(limits, Retained::retained_bytes, wake)
}

/// Creates a mailbox using a caller-selected deterministic measurement function.
pub fn mailbox_with<T>(
    limits: MailboxLimits,
    measure: fn(&T) -> RetainedBytes,
    wake: WakeHandle,
) -> (MailboxSender<T>, MailboxReceiver<T>) {
    let shared = Arc::new(Shared {
        limits,
        state: Mutex::new(State::new(limits)),
        counters: Counters::new(),
        measure,
        wake,
    });
    (
        MailboxSender {
            shared: Arc::clone(&shared),
        },
        MailboxReceiver {
            shared,
            _single_consumer: PhantomData,
        },
    )
}
