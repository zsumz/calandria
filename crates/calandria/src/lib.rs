//! Bounded reactor and single-owner duty primitives.
//!
//! Calandria separates reusable execution mechanisms from domain policy. The
//! core vocabulary and embedded duty host are available without allocation;
//! owner-local queues and tables use the optional `alloc` feature, enabled by
//! default through `std`.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
pub mod batch;
#[cfg(feature = "std")]
pub mod completion;
pub mod host;
#[cfg(feature = "alloc")]
pub mod io;
#[cfg(feature = "std")]
pub mod mailbox;
#[cfg(feature = "std")]
pub mod reactor;
#[cfg(feature = "alloc")]
pub mod resource;
pub mod retained;
#[cfg(feature = "std")]
pub mod shutdown;
pub mod time;
#[cfg(feature = "alloc")]
pub mod timer;
pub mod turn;
#[cfg(feature = "std")]
pub mod wake;

#[cfg(feature = "alloc")]
pub use batch::{
    EventBatch, EventBatchDrain, EventBatchError, EventBatchFailure, EventBatchLimits,
    EventBatchSnapshot,
};
#[cfg(feature = "std")]
pub use completion::{Completer, Completion, CompletionError, completion};
pub use host::{
    Clock, Duty, EmbeddedHost, HostAction, HostConfig, HostError, HostPhase, HostSnapshot, HostStep,
};
#[cfg(feature = "std")]
pub use host::{MonotonicClock, ThreadNotifier, ThreadParker, WaitOutcome, Waiter, thread_parker};
#[cfg(feature = "alloc")]
pub use io::{
    Interest, PollEvent, PollEvents, PollEventsDrain, PollEventsError, PollReport, Poller,
    Readiness,
};
#[cfg(feature = "std")]
pub use mailbox::{
    AdmissionFailure, DrainReport, DrainStatus, Lane, LaneLimits, LaneSnapshot, MailboxLimits,
    MailboxReceiver, MailboxSender, MailboxSnapshot, TrySendError, mailbox, mailbox_with,
};
#[cfg(feature = "std")]
pub use reactor::{
    Reactor, ReactorExit, ReactorFailure, ReactorGroup, ReactorGroupExit, ReactorGroupHandle,
    ReactorGroupLimits, ReactorGroupMember, ReactorGroupMemberExit, ReactorGroupOutcome,
    ReactorGroupSendError, ReactorGroupSendFailure, ReactorGroupSpawnError,
    ReactorGroupSpawnFailure, ReactorGroupTermination, ReactorHandle, ReactorId, ReactorOutcome,
    ReactorSnapshot, ReactorSpawnError, ReactorTermination, ReactorTerminationStatus,
};
#[cfg(feature = "alloc")]
pub use resource::{
    ResourceAdmissionError, ResourceAdmissionFailure, ResourceGeneration, ResourceOwnerId,
    ResourceSlotId, ResourceTable, ResourceTableSnapshot, ResourceToken, ResourceTokenFailure,
};
pub use retained::{Retained, RetainedBytes, RetainedBytesOverflow};
#[cfg(feature = "std")]
pub use shutdown::{
    ShutdownCompleter, ShutdownRequester, ShutdownSubscribeError, shutdown_barrier,
};
pub use time::{Deadline, DurationOverflow, Moment, Span};
#[cfg(feature = "alloc")]
pub use timer::{
    Timer, TimerDrain, TimerId, TimerLimits, TimerOwnerId, TimerQueue, TimerQueueSnapshot,
    TimerScheduleError, TimerScheduleFailure, TimerToken,
};
pub use turn::{Next, Turn, WorkCount};
#[cfg(feature = "std")]
pub use wake::{WakeHandle, WakeSource};
