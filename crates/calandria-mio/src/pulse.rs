//! Acknowledgement-free publication pulses for one Mio selector.

use std::{fmt, io, sync::Arc};

use mio::Waker;

/// Cloneable handle that invokes one Mio selector waker per pulse call.
///
/// This handle performs no Calandria-side coalescing and retains no
/// acknowledgement state. Mio may still combine multiple pending wakeups into
/// one readiness event. A producer must publish durable state before pulsing,
/// and the selector owner must drain or rescan that state after notification.
#[derive(Clone)]
pub struct MioPulseHandle {
    waker: Arc<Waker>,
}

impl MioPulseHandle {
    pub(crate) fn new(waker: Arc<Waker>) -> Self {
        Self { waker }
    }

    /// Invokes the underlying Mio waker without Calandria-side coalescing.
    pub fn pulse(&self) -> io::Result<()> {
        self.waker.wake()
    }
}

impl fmt::Debug for MioPulseHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("MioPulseHandle").finish()
    }
}
