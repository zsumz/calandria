//! Resumable external delivery injection into active deterministic worlds.

use calandria::{Moment, Span};

use crate::{DutyId, EventToken, Model, Routed, Scheduler};

use super::{
    InjectionError, InjectionFailure, Monitor, Simulation, SimulationPhase,
};

impl<M, S, N> Simulation<M, S, N>
where
    M: Model,
    S: Scheduler,
    N: Monitor<M>,
{
    /// Injects an immediate modeled delivery with no parent action.
    pub fn inject(
        &mut self,
        target: DutyId,
        event: M::Event,
    ) -> Result<EventToken, InjectionError<M::Event>> {
        self.inject_at(target, self.now(), event)
    }

    /// Injects a modeled delivery after a relative virtual delay.
    pub fn inject_after(
        &mut self,
        target: DutyId,
        delay: Span,
        event: M::Event,
    ) -> Result<EventToken, InjectionError<M::Event>> {
        let Some(at) = self.now().checked_add(delay) else {
            return Err(InjectionError::new(
                event,
                InjectionFailure::TimeOverflow {
                    current: self.now(),
                    delay,
                },
            ));
        };
        self.inject_at(target, at, event)
    }

    /// Injects a modeled delivery at an absolute virtual moment.
    pub fn inject_at(
        &mut self,
        target: DutyId,
        at: Moment,
        event: M::Event,
    ) -> Result<EventToken, InjectionError<M::Event>> {
        if self.poisoned.get() {
            return Err(InjectionError::new(event, InjectionFailure::Poisoned));
        }
        if self.phase != SimulationPhase::Active {
            return Err(InjectionError::new(
                event,
                InjectionFailure::Inactive(self.phase),
            ));
        }
        if !self.topology.contains(target) {
            return Err(InjectionError::new(
                event,
                InjectionFailure::UnknownTarget(target),
            ));
        }
        if self.stopped.contains(&target) {
            return Err(InjectionError::new(
                event,
                InjectionFailure::TargetStopped(target),
            ));
        }
        if at > self.limits.max_virtual_time() {
            return Err(InjectionError::new(
                event,
                InjectionFailure::BeyondTimeLimit {
                    requested: at,
                    limit: self.limits.max_virtual_time(),
                },
            ));
        }

        let routed = Routed::new(target, None, event);
        match self.timeline.schedule_at(at, routed) {
            Ok(token) => Ok(token),
            Err(error) => {
                let (routed, failure) = error.into_parts();
                Err(InjectionError::new(
                    routed.event,
                    InjectionFailure::Timeline(failure),
                ))
            }
        }
    }
}
