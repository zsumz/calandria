//! Fixed-size committed action facts retained by one causal trace.

use calandria::{Retained, RetainedBytes, Turn};

use crate::{ActionMeta, ActionRecord};

/// Exact metadata and scheduling interest committed by one model action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TraceEntry {
    meta: ActionMeta,
    turn: Turn,
}

impl TraceEntry {
    pub(crate) const fn from_record<O>(record: &ActionRecord<O>) -> Self {
        Self {
            meta: record.meta(),
            turn: record.turn(),
        }
    }

    pub(crate) const fn new(meta: ActionMeta, turn: Turn) -> Self {
        Self { meta, turn }
    }

    /// Returns exact action identity, selection, time, and causal parent.
    pub const fn meta(self) -> ActionMeta {
        self.meta
    }

    /// Returns complete scheduling interest committed by the action.
    pub const fn turn(self) -> Turn {
        self.turn
    }
}

impl Retained for TraceEntry {
    fn retained_bytes(&self) -> RetainedBytes {
        RetainedBytes::ZERO
    }
}
