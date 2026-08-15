//! Atomic group admission phase with independent per-shard publication fences.

use std::{
    num::NonZeroUsize,
    sync::{
        Mutex, MutexGuard,
        atomic::{AtomicBool, Ordering},
    },
};

use crate::MailboxSender;

pub(super) struct Shared<T> {
    pub(super) reactors: NonZeroUsize,
    pub(super) shards: Box<[Shard<T>]>,
    open: AtomicBool,
}

impl<T> Shared<T> {
    pub(super) fn new(senders: Vec<MailboxSender<T>>) -> Self {
        Self {
            reactors: NonZeroUsize::new(senders.len())
                .unwrap_or_else(|| panic!("validated reactor group topology became empty")),
            open: AtomicBool::new(true),
            shards: senders
                .into_iter()
                .map(Shard::new)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        }
    }

    pub(super) fn is_open(&self) -> bool {
        self.open.load(Ordering::Acquire)
    }

    pub(super) fn close(&self) {
        self.open.store(false, Ordering::Release);
        for shard in &self.shards {
            drop(shard.lock());
        }
    }
}

pub(super) struct Shard<T> {
    admission: Mutex<()>,
    pub(super) ingress: MailboxSender<T>,
}

impl<T> Shard<T> {
    fn new(ingress: MailboxSender<T>) -> Self {
        Self {
            admission: Mutex::new(()),
            ingress,
        }
    }

    pub(super) fn lock(&self) -> MutexGuard<'_, ()> {
        self.admission
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}
