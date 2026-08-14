//! Deterministic execution and virtual capabilities for Calandria duties.
//!
//! The crate owns one virtual-time domain, bounded typed deliveries, explicit
//! owner turns, deterministic scheduling, transactional effects, and immutable
//! post-action monitoring. It performs no I/O, starts no threads, reads no wall
//! clock, and obtains no ambient entropy.

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod action;
pub mod clock;
pub mod event;
pub mod model;
pub mod plan;
pub mod scheduler;
pub mod script;
pub mod simulation;
pub mod timeline;

pub use action::{ActionId, ActionKey, ActionKind, ActionMeta, ActionRecord, ReadySet};
pub use clock::{ClockError, VirtualClock};
pub use event::{Delivery, EventId, EventToken, TimelineId};
pub(crate) use model::Routed;
pub use model::{
    ActionContext, CancelFailure, DutyId, Model, ObservationError, ObservationFailure, SendError,
    SendFailure, Topology, TopologyError,
};
pub use plan::Planned;
pub use scheduler::{Fifo, RoundRobin, Scheduler};
pub use script::{
    ExactScript, Plan, ScriptBuildError, ScriptBuildFailure, ScriptFailure, ScriptLimits,
    ScriptStep,
};
pub use simulation::{
    DutySnapshot, InjectionError, InjectionFailure, KernelFailure, LimitFailure, Monitor,
    NoopMonitor, RunEnd, RunError, RunReport, Simulation, SimulationBuildError, SimulationLimits,
    SimulationPhase, SimulationSnapshot, SimulationView, Step, StepError,
};
pub use timeline::{ScheduleError, ScheduleFailure, Timeline, TimelineLimits, TimelineSnapshot};
