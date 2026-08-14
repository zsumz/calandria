//! Bounded generational ownership of owner-local resources.

mod access;
mod error;
mod identity;
mod lifecycle;
mod slot;
mod snapshot;
mod table;
mod validation;

pub use error::{ResourceAdmissionError, ResourceAdmissionFailure, ResourceTokenFailure};
pub use identity::{ResourceGeneration, ResourceOwnerId, ResourceSlotId, ResourceToken};
pub use snapshot::ResourceTableSnapshot;
pub use table::ResourceTable;

use slot::{Slot, token_failure};
