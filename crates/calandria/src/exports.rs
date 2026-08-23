//! Crate-root public vocabulary re-exported from its owning domain modules.

#[cfg(feature = "alloc")]
pub use crate::batch::{
    EventBatch, EventBatchDrain, EventBatchError, EventBatchFailure, EventBatchLimits,
    EventBatchSnapshot,
};
#[cfg(feature = "std")]
pub use crate::completion::{Completer, Completion, CompletionError, completion};
pub use crate::host::{
    Clock, Duty, EmbeddedHost, HostAction, HostConfig, HostError, HostPhase, HostSnapshot, HostStep,
};
#[cfg(feature = "std")]
pub use crate::host::{
    MonotonicClock, ThreadNotifier, ThreadParker, WaitOutcome, Waiter, thread_parker,
};
#[cfg(feature = "alloc")]
pub use crate::io::{
    Interest, PollEvent, PollEvents, PollEventsDrain, PollEventsError, PollReport, Poller,
    Readiness,
};
#[cfg(feature = "std")]
pub use crate::mailbox::{
    AdmissionFailure, DrainReport, DrainStatus, Lane, LaneLimits, LaneSnapshot, MailboxLimits,
    MailboxReceiver, MailboxSender, MailboxSnapshot, TrySendError, mailbox, mailbox_with,
};
#[cfg(feature = "std")]
pub use crate::reactor::{
    Reactor, ReactorExit, ReactorFailure, ReactorGroup, ReactorGroupExit, ReactorGroupHandle,
    ReactorGroupLimits, ReactorGroupMember, ReactorGroupMemberExit, ReactorGroupOutcome,
    ReactorGroupSendError, ReactorGroupSendFailure, ReactorGroupSpawnError,
    ReactorGroupSpawnFailure, ReactorGroupTermination, ReactorHandle, ReactorId, ReactorOutcome,
    ReactorSnapshot, ReactorSpawnError, ReactorTermination, ReactorTerminationStatus,
};
#[cfg(feature = "alloc")]
pub use crate::resource::{
    ResourceAdmissionError, ResourceAdmissionFailure, ResourceGeneration, ResourceOwnerId,
    ResourceSlotId, ResourceTable, ResourceTableSnapshot, ResourceToken, ResourceTokenFailure,
};
pub use crate::retained::{Retained, RetainedBytes, RetainedBytesOverflow};
#[cfg(feature = "std")]
pub use crate::shutdown::{
    ShutdownCompleter, ShutdownRequester, ShutdownSubscribeError, shutdown_barrier,
};
pub use crate::time::{Deadline, DurationOverflow, Moment, Span};
#[cfg(feature = "alloc")]
pub use crate::timer::{
    Timer, TimerDrain, TimerId, TimerLimits, TimerOwnerId, TimerQueue, TimerQueueSnapshot,
    TimerScheduleError, TimerScheduleFailure, TimerToken,
};
pub use crate::turn::{Next, Turn, WorkCount};
#[cfg(feature = "std")]
pub use crate::wake::{WakeHandle, WakeSource};
