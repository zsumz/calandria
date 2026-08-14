//! Deterministic one- and many-duty execution over one virtual-time domain.

mod error;
mod execute;
mod injection;
mod limits;
mod monitor;
mod outcome;
mod owner;
mod poison;
mod run;
mod state;
mod step;
mod view;

pub use error::{
    InjectionError, InjectionFailure, KernelFailure, LimitFailure,
    SimulationBuildError, StepError,
};
pub use limits::SimulationLimits;
pub use monitor::{Monitor, NoopMonitor};
pub use outcome::{RunEnd, RunError, RunReport, Step};
pub use owner::Simulation;
pub(crate) use poison::PoisonGuard;
pub use state::{DutySnapshot, SimulationPhase, SimulationSnapshot};
pub use view::SimulationView;

pub(crate) use state::DutyState;
