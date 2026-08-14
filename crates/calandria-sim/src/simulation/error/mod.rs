//! Construction, injection, kernel, limit, and action failure taxonomy.

mod build;
mod injection;
mod kernel;
mod step;

pub use build::SimulationBuildError;
pub use injection::{InjectionError, InjectionFailure};
pub use kernel::{KernelFailure, LimitFailure};
pub use step::StepError;
