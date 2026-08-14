//! Panic poisoning across consumer-controlled execution boundaries.

use core::cell::Cell;

pub(crate) struct PoisonGuard<'a> {
    poisoned: &'a Cell<bool>,
}

impl<'a> PoisonGuard<'a> {
    pub(crate) fn new(poisoned: &'a Cell<bool>) -> Self {
        poisoned.set(true);
        Self { poisoned }
    }

    pub(crate) fn disarm(self) {
        self.poisoned.set(false);
    }
}
