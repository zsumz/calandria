//! Linearized external termination state for one reactor owner.

use std::sync::{Arc, Mutex, MutexGuard};

use crate::WakeHandle;

use super::{ReactorTermination, ReactorTerminationStatus};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Running,
    TerminationRequested,
    Exited,
}

pub(crate) struct ReactorControl {
    shared: Arc<Shared>,
}

pub(super) struct ReactorControlOwner {
    shared: Arc<Shared>,
}

pub(super) struct PublishedTermination {
    status: ReactorTerminationStatus,
    wake: Option<WakeHandle>,
}

pub(super) fn control_pair(wake: WakeHandle) -> (ReactorControlOwner, ReactorControl) {
    let shared = Arc::new(Shared {
        phase: Mutex::new(Phase::Running),
        wake,
    });
    (
        ReactorControlOwner {
            shared: Arc::clone(&shared),
        },
        ReactorControl { shared },
    )
}

impl ReactorControl {
    pub(super) fn request_termination(&self) -> ReactorTermination {
        self.publish_termination().complete()
    }

    pub(super) fn publish_termination(&self) -> PublishedTermination {
        let mut phase = self.shared.lock();
        match *phase {
            Phase::Running => {
                *phase = Phase::TerminationRequested;
                PublishedTermination {
                    status: ReactorTerminationStatus::Requested,
                    wake: Some(self.shared.wake.clone()),
                }
            }
            Phase::TerminationRequested => PublishedTermination {
                status: ReactorTerminationStatus::AlreadyRequested,
                wake: None,
            },
            Phase::Exited => PublishedTermination {
                status: ReactorTerminationStatus::Exited,
                wake: None,
            },
        }
    }
}

impl PublishedTermination {
    pub(super) fn complete(self) -> ReactorTermination {
        let wake_error = self.wake.and_then(|wake| wake.wake().err());
        ReactorTermination::new(self.status, wake_error)
    }
}

impl Clone for ReactorControl {
    fn clone(&self) -> Self {
        Self {
            shared: Arc::clone(&self.shared),
        }
    }
}

impl ReactorControlOwner {
    pub(super) fn control(&self) -> ReactorControl {
        ReactorControl {
            shared: Arc::clone(&self.shared),
        }
    }

    pub(super) fn termination_requested(&self) -> bool {
        *self.shared.lock() == Phase::TerminationRequested
    }

    pub(super) fn finish(&self) -> bool {
        let mut phase = self.shared.lock();
        let termination_requested = *phase == Phase::TerminationRequested;
        *phase = Phase::Exited;
        termination_requested
    }
}

impl Drop for ReactorControlOwner {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

struct Shared {
    phase: Mutex<Phase>,
    wake: WakeHandle,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, Phase> {
        self.phase
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl core::fmt::Debug for ReactorControl {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ReactorControl")
            .field("phase", &*self.shared.lock())
            .finish_non_exhaustive()
    }
}

impl core::fmt::Debug for ReactorControlOwner {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ReactorControlOwner")
            .field("phase", &*self.shared.lock())
            .finish_non_exhaustive()
    }
}
