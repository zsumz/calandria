//! Production synchronization selected for ordinary and Loom builds.

#[cfg(calandria_loom)]
pub(crate) use loom::sync::{Arc, Condvar, Mutex, MutexGuard};
#[cfg(not(calandria_loom))]
pub(crate) use std::sync::{Arc, Condvar, Mutex, MutexGuard};

#[cfg(calandria_loom)]
pub(crate) mod atomic {
    pub(crate) use loom::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
}

#[cfg(not(calandria_loom))]
pub(crate) mod atomic {
    pub(crate) use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
}

pub(crate) fn spin_loop() {
    #[cfg(calandria_loom)]
    loom::thread::yield_now();
    #[cfg(not(calandria_loom))]
    std::hint::spin_loop();
}

pub(crate) fn yield_now() {
    #[cfg(calandria_loom)]
    loom::thread::yield_now();
    #[cfg(not(calandria_loom))]
    std::thread::yield_now();
}

pub(crate) fn recover_poison<T>(error: std::sync::PoisonError<T>) -> T {
    error.into_inner()
}
