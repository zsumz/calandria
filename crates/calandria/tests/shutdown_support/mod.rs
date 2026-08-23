//! Concurrent subscriber construction and fixed limits for shutdown tests.

use std::{
    num::NonZeroUsize,
    sync::{
        Arc, Barrier,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
};

pub(crate) fn spawn_subscriber(
    requester: &Arc<calandria::ShutdownRequester>,
    gate: &Arc<Barrier>,
    requests: &Arc<AtomicUsize>,
) -> thread::JoinHandle<calandria::Completion<()>> {
    let requester = Arc::clone(requester);
    let gate = Arc::clone(gate);
    let requests = Arc::clone(requests);
    thread::spawn(move || {
        gate.wait();
        requester
            .subscribe(|| {
                requests.fetch_add(1, Ordering::SeqCst);
                Ok::<_, ()>(())
            })
            .unwrap_or_else(|_| panic!("admit concurrent shutdown observer"))
    })
}

pub(crate) fn nonzero(value: usize) -> NonZeroUsize {
    NonZeroUsize::new(value).unwrap_or_else(|| panic!("test capacity must be nonzero"))
}
