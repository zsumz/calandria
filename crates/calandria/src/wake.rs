//! Coalesced cross-thread notification for reactor progress.

use std::{
    fmt, io,
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
    thread,
};

const IDLE: u8 = 0;
const WAKING: u8 = 1;
const REQUESTED: u8 = 2;
const WAKING_ACKNOWLEDGED: u8 = 3;
const SPIN_LIMIT: usize = 64;

/// Backend capable of interrupting a blocked reactor owner.
pub trait WakeSource: Send + Sync + 'static {
    /// Requests that the owner return from its current wait.
    ///
    /// Implementations must be nonblocking, must not panic, and must not call
    /// back into the publication or termination path that invoked them.
    /// Calandria serializes concurrent requests while this method is in
    /// progress so a failed request cannot be mistaken for a successful
    /// coalesced wake.
    fn wake(&self) -> io::Result<()>;
}

impl<F> WakeSource for F
where
    F: Fn() -> io::Result<()> + Send + Sync + 'static,
{
    fn wake(&self) -> io::Result<()> {
        self()
    }
}

/// Cloneable notification handle that coalesces repeated wake requests.
#[derive(Clone)]
pub struct WakeHandle {
    shared: Arc<WakeState>,
}

impl WakeHandle {
    /// Creates a coalesced handle around one backend wake source.
    pub fn new(source: impl WakeSource) -> Self {
        Self {
            shared: Arc::new(WakeState {
                phase: AtomicU8::new(IDLE),
                source: Box::new(source),
            }),
        }
    }

    /// Requests owner progress.
    ///
    /// Only the first successful request after an acknowledgement reaches the
    /// backend. Concurrent callers wait for an in-flight backend request to
    /// resolve. If that request fails, one caller may retry rather than
    /// returning success for a wake that never occurred.
    pub fn wake(&self) -> io::Result<()> {
        let mut spins = 0;
        loop {
            match self.shared.phase.load(Ordering::Acquire) {
                REQUESTED => return Ok(()),
                IDLE => {
                    if self
                        .shared
                        .phase
                        .compare_exchange(IDLE, WAKING, Ordering::AcqRel, Ordering::Acquire)
                        .is_err()
                    {
                        continue;
                    }

                    return self.finish_wake(self.shared.source.wake());
                }
                WAKING | WAKING_ACKNOWLEDGED => {
                    if spins < SPIN_LIMIT {
                        spins += 1;
                        std::hint::spin_loop();
                    } else {
                        thread::yield_now();
                    }
                }
                _ => panic!("wake phase invariant violated"),
            }
        }
    }

    /// Acknowledges that the single owner has observed all work preceding the wake.
    ///
    /// # Contract
    ///
    /// Only the owner of the associated reactor may call this method. A queue
    /// must pair acknowledgement with the same lock that linearizes
    /// publication, preventing a producer from publishing accepted work
    /// between the owner's empty check and this acknowledgement. Calling it
    /// from a producer or without that synchronization can strand work.
    pub fn acknowledge(&self) {
        loop {
            match self.shared.phase.load(Ordering::Acquire) {
                IDLE | WAKING_ACKNOWLEDGED => return,
                REQUESTED => {
                    if self
                        .shared
                        .phase
                        .compare_exchange(REQUESTED, IDLE, Ordering::AcqRel, Ordering::Acquire)
                        .is_ok()
                    {
                        return;
                    }
                }
                WAKING => {
                    if self
                        .shared
                        .phase
                        .compare_exchange(
                            WAKING,
                            WAKING_ACKNOWLEDGED,
                            Ordering::AcqRel,
                            Ordering::Acquire,
                        )
                        .is_ok()
                    {
                        return;
                    }
                }
                _ => panic!("wake phase invariant violated"),
            }
        }
    }

    /// Returns whether a wake is requested or being issued.
    pub fn is_requested(&self) -> bool {
        self.shared.phase.load(Ordering::Acquire) != IDLE
    }

    fn finish_wake(&self, result: io::Result<()>) -> io::Result<()> {
        match result {
            Ok(()) => {
                match self.shared.phase.compare_exchange(
                    WAKING,
                    REQUESTED,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                ) {
                    Ok(_) => {}
                    Err(WAKING_ACKNOWLEDGED) => {
                        self.shared.phase.store(IDLE, Ordering::Release);
                    }
                    Err(actual) => {
                        panic!("wake completion invariant violated at phase {actual}")
                    }
                }
                Ok(())
            }
            Err(source) => {
                let previous = self.shared.phase.swap(IDLE, Ordering::AcqRel);
                assert!(
                    previous == WAKING || previous == WAKING_ACKNOWLEDGED,
                    "wake failure invariant violated"
                );
                Err(source)
            }
        }
    }
}

impl fmt::Debug for WakeHandle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WakeHandle")
            .field("requested", &self.is_requested())
            .finish_non_exhaustive()
    }
}

struct WakeState {
    phase: AtomicU8,
    source: Box<dyn WakeSource>,
}
