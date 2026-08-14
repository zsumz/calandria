//! Canonical ready-set construction, scheduling, and virtual-time advancement.

use calandria::Next;

use crate::{ActionId, ActionKey, Model, ReadySet, Scheduler};

use super::{
    KernelFailure, LimitFailure, Monitor, PoisonGuard, Simulation, SimulationPhase, Step, StepError,
};

type StepResult<O, ME, NE, SE> = Result<Step<O>, StepError<ME, NE, SE>>;
type StepGuardResult<ME, NE, SE> = Result<(), StepError<ME, NE, SE>>;

impl<M, S, N> Simulation<M, S, N>
where
    M: Model,
    S: Scheduler,
    N: Monitor<M>,
{
    /// Executes one model action, time advance, or terminal observation.
    pub fn step(&mut self) -> StepResult<M::Observation, M::Error, N::Error, S::Error> {
        if self.poisoned.get() {
            return Err(StepError::Poisoned);
        }
        if self.phase != SimulationPhase::Active {
            return Err(StepError::Inactive(self.phase));
        }
        if let Err(failure) = self.build_ready() {
            self.fail();
            return Err(StepError::Kernel(failure));
        }
        if self.ready.is_empty() {
            return self.advance_or_finish();
        }
        if let Err(error) = self.enforce_action_limits() {
            self.fail();
            return Err(error);
        }

        let ready = ReadySet::new(&self.ready);
        let guard = PoisonGuard::new(&self.poisoned);
        let choice = self.scheduler.choose(self.now(), ready);
        guard.disarm();
        let selected = match choice {
            Ok(selected) => selected,
            Err(source) => {
                self.fail();
                return Err(StepError::Scheduler(source));
            }
        };
        if !ready.contains(selected) {
            self.fail();
            return Err(StepError::InvalidSelection(selected));
        }

        let id = match self.allocate_action() {
            Ok(id) => id,
            Err(failure) => {
                self.fail();
                return Err(StepError::Kernel(failure));
            }
        };
        self.actions += 1;
        self.actions_at_moment += 1;
        self.execute_selected(selected, id).map(Step::Action)
    }

    fn build_ready(&mut self) -> Result<(), KernelFailure> {
        self.ready.clear();
        let now = self.now();
        let timeline = &self.timeline;
        let topology = &self.topology;
        let stopped = &self.stopped;
        let ready = &mut self.ready;
        for token in timeline.tokens_at(now) {
            let routed = timeline
                .event(token)
                .unwrap_or_else(|| panic!("enumerated event must remain pending"));
            if !topology.contains(routed.target) {
                return Err(KernelFailure::UnknownDeliveryTarget {
                    token,
                    target: routed.target,
                });
            }
            if stopped.contains(&routed.target) {
                return Err(KernelFailure::DeliveryTargetsStopped {
                    token,
                    target: routed.target,
                });
            }
            ready.push(ActionKey::delivery(routed.target, token));
        }
        for (duty, state) in &self.states {
            let runnable = match state.next {
                Next::Now => true,
                Next::WakeOr(deadline) => deadline.is_elapsed_at(now),
                Next::Wake | Next::Stop => false,
            };
            if runnable {
                ready.push(ActionKey::turn(*duty));
            }
        }
        ready.sort_unstable();
        ready.dedup();
        Ok(())
    }

    fn advance_or_finish(&mut self) -> StepResult<M::Observation, M::Error, N::Error, S::Error> {
        if self.stopped.len() == self.topology.len() {
            let pending = self
                .timeline
                .pending()
                .next()
                .map(|(token, routed)| (token, routed.target));
            if let Some((token, target)) = pending {
                let failure = KernelFailure::DeliveryTargetsStopped { token, target };
                self.fail();
                return Err(StepError::Kernel(failure));
            }
            self.validate_scheduler_finish()?;
            self.phase = SimulationPhase::Completed;
            return Ok(Step::Completed(self.snapshot()));
        }

        let now = self.now();
        let next = earliest(self.timeline.next_at(), self.next_deadline());
        let Some(next) = next else {
            return Ok(Step::Quiescent(self.snapshot()));
        };
        if next <= now {
            self.fail();
            return Err(StepError::Kernel(KernelFailure::NoFutureProgress { now }));
        }
        if next > self.limits.max_virtual_time() {
            let failure = LimitFailure::VirtualTime {
                next,
                limit: self.limits.max_virtual_time(),
            };
            self.fail();
            return Err(StepError::Limit(failure));
        }
        self.timeline.advance_to(next);
        self.actions_at_moment = 0;
        Ok(Step::TimeAdvanced {
            from: now,
            to: next,
        })
    }

    fn next_deadline(&self) -> Option<calandria::Moment> {
        self.states
            .values()
            .filter_map(|state| match state.next {
                Next::WakeOr(deadline) => Some(deadline.moment()),
                Next::Now | Next::Wake | Next::Stop => None,
            })
            .min()
    }

    fn enforce_action_limits(&self) -> StepGuardResult<M::Error, N::Error, S::Error> {
        if self.actions >= self.limits.total_actions().get() {
            return Err(StepError::Limit(LimitFailure::TotalActions {
                limit: self.limits.total_actions(),
            }));
        }
        if self.actions_at_moment >= self.limits.actions_per_moment().get() {
            return Err(StepError::Limit(LimitFailure::ActionsAtMoment {
                at: self.now(),
                limit: self.limits.actions_per_moment(),
            }));
        }
        Ok(())
    }

    fn allocate_action(&mut self) -> Result<ActionId, KernelFailure> {
        let Some(raw) = self.next_action else {
            return Err(KernelFailure::ActionIdsExhausted);
        };
        self.next_action = raw.checked_add(1);
        Ok(ActionId::from_raw(raw))
    }

    fn validate_scheduler_finish(&mut self) -> StepGuardResult<M::Error, N::Error, S::Error> {
        let guard = PoisonGuard::new(&self.poisoned);
        let result = self.scheduler.finished();
        guard.disarm();
        match result {
            Ok(()) => Ok(()),
            Err(source) => {
                self.fail();
                Err(StepError::Scheduler(source))
            }
        }
    }
}

fn earliest(
    left: Option<calandria::Moment>,
    right: Option<calandria::Moment>,
) -> Option<calandria::Moment> {
    match (left, right) {
        (Some(left), Some(right)) => Some(core::cmp::min(left, right)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}
