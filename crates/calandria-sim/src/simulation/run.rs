//! Whole-world execution until quiescence or clean completion.

use crate::{Model, Scheduler};

use super::{Monitor, RunEnd, RunError, RunReport, Simulation, Step, StepError};

type QuiescenceResult<ME, NE, SE> = Result<RunReport, StepError<ME, NE, SE>>;
type CompletionResult<ME, NE, SE> = Result<RunReport, RunError<StepError<ME, NE, SE>>>;

impl<M, S, N> Simulation<M, S, N>
where
    M: Model,
    S: Scheduler,
    N: Monitor<M>,
{
    /// Runs until clean completion or intentional quiescence.
    pub fn run_to_quiescence(&mut self) -> QuiescenceResult<M::Error, N::Error, S::Error> {
        loop {
            match self.step()? {
                Step::Action(_) | Step::TimeAdvanced { .. } => {}
                Step::Quiescent(snapshot) => {
                    return Ok(RunReport::new(RunEnd::Quiescent, snapshot));
                }
                Step::Completed(snapshot) => {
                    return Ok(RunReport::new(RunEnd::Completed, snapshot));
                }
            }
        }
    }

    /// Runs until every duty stops, treating quiescence as deadlock.
    pub fn run_to_completion(&mut self) -> CompletionResult<M::Error, N::Error, S::Error> {
        match self.run_to_quiescence().map_err(RunError::Step)? {
            report if report.end() == RunEnd::Completed => Ok(report),
            report => Err(RunError::Deadlock(report.snapshot())),
        }
    }
}
