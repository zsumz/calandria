//! Count- and byte-bounded multi-producer, single-consumer reactor mailbox.

mod config;
mod error;
mod factory;
mod receiver;
mod sender;
mod shared;
mod snapshot;

pub use config::{Lane, LaneLimits, MailboxLimits};
pub use error::{AdmissionFailure, TrySendError};
pub use factory::{mailbox, mailbox_with};
pub use receiver::MailboxReceiver;
pub use sender::MailboxSender;
pub use snapshot::{DrainReport, DrainStatus, LaneSnapshot, MailboxSnapshot};
