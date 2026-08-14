//! Caller-driven execution of one bounded duty.

use crate::Moment;

use super::{
    Clock, Duty, HostAction, HostConfig, HostError, HostPhase, HostSnapshot, HostStep,
};

/// Embedded host whose caller owns every scheduling opportunity.
#[derive(Debug)]
pub struct EmbeddedHost<D, C> {
    duty: D,
    clock: C,
    config: HostConfig,
    snapshot: HostSnapshot,
}

impl<D, C> EmbeddedHost<D, C>
where
    D: Duty,
    C: Clock,
{
    /// Creates an embedded host around one owned duty and clock.
    pub const fn new(duty: D, clock: C, config: HostConfig) -> Self {
        Self {
            duty,
            clock,
            config,
            snapshot: HostSnapshot::new(),
        }
    }

    /// Executes exactly one bounded duty turn and selects the next action.
    pub fn step(&mut self) -> Result<HostStep, HostError<D::Error, C::Error>> {
        if self.snapshot.phase() != HostPhase::Running {
            return Err(HostError::NotRunning {
                phase: self.snapshot.phase(),
            });
        }

        let started_at = self.observe_now()?;
        let turn = match self.duty.turn(started_at) {
            Ok(turn) => turn,
            Err(source) => {
                self.snapshot.fail();
                return Err(HostError::Duty(source));
            }
        };
        let completed_at = self.observe_now()?;
        let action = HostAction::for_turn(turn, completed_at, self.config);
        self.snapshot.record(turn, action);
        if action == HostAction::Stop {
            self.snapshot.stop();
        }

        Ok(HostStep::new(started_at, completed_at, turn, action))
    }

    /// Returns current host diagnostics.
    pub const fn snapshot(&self) -> HostSnapshot {
        self.snapshot
    }

    /// Returns shared access to the owned duty.
    pub const fn duty(&self) -> &D {
        &self.duty
    }

    /// Returns exclusive access while the embedding caller owns the host.
    pub fn duty_mut(&mut self) -> &mut D {
        &mut self.duty
    }

    /// Returns shared access to the owned clock.
    pub const fn clock(&self) -> &C {
        &self.clock
    }

    /// Returns exclusive access to the owned clock.
    pub fn clock_mut(&mut self) -> &mut C {
        &mut self.clock
    }

    /// Returns the immutable host policy.
    pub const fn config(&self) -> HostConfig {
        self.config
    }

    /// Consumes the host and returns its owned components and final snapshot.
    pub fn into_parts(self) -> (D, C, HostConfig, HostSnapshot) {
        (self.duty, self.clock, self.config, self.snapshot)
    }

    /// Consumes the host and returns the duty.
    pub fn into_duty(self) -> D {
        self.duty
    }

    pub(super) fn fail(&mut self) {
        self.snapshot.fail();
    }

    fn observe_now(&mut self) -> Result<Moment, HostError<D::Error, C::Error>> {
        let observed = match self.clock.now() {
            Ok(observed) => observed,
            Err(source) => {
                self.snapshot.fail();
                return Err(HostError::Clock(source));
            }
        };

        if let Some(previous) = self.snapshot.last_moment()
            && observed < previous
        {
            self.snapshot.fail();
            return Err(HostError::ClockRegressed { previous, observed });
        }

        self.snapshot.observe(observed);
        Ok(observed)
    }
}
