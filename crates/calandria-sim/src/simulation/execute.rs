//! Exact owner turn and event-delivery execution with transactional effects.

use alloc::vec::Vec;

use calandria::{Next, Retained, Turn};

use crate::{
    ActionContext, ActionId, ActionKey, ActionKind, ActionMeta, ActionRecord, Delivery,
    Model, Scheduler,
};

use super::{
    KernelFailure, Monitor, PoisonGuard, Simulation, SimulationView, StepError,
};

impl<M, S, N> Simulation<M, S, N>
where
    M: Model,
    S: Scheduler,
    N: Monitor<M>,
{
    pub(super) fn execute_selected(
        &mut self,
        key: ActionKey,
        id: ActionId,
    ) -> Result<ActionRecord<M::Observation>, StepError<M::Error, N::Error, S::Error>> {
        match key.kind() {
            ActionKind::Delivery(token) => self.execute_delivery(key, id, token),
            ActionKind::Turn => self.execute_turn(key, id),
        }
    }

    fn execute_turn(
        &mut self,
        key: ActionKey,
        id: ActionId,
    ) -> Result<ActionRecord<M::Observation>, StepError<M::Error, N::Error, S::Error>> {
        let now = self.now();
        let meta = ActionMeta::new(id, key, now, None);
        let mut context = ActionContext::new(
            &mut self.timeline,
            &self.topology,
            &self.stopped,
            id,
            now,
            self.limits.action_context(),
        );
        let guard = PoisonGuard::new(&self.poisoned);
        let result = self.model.turn(key.duty(), now, &mut context);
        guard.disarm();
        let turn = match result {
            Ok(turn) => turn,
            Err(source) => {
                drop(context);
                self.fail();
                return Err(StepError::Model {
                    action: meta,
                    source,
                });
            }
        };
        let observations = match commit_context(key.duty(), turn, context) {
            Ok(observations) => observations,
            Err(failure) => {
                self.fail();
                return Err(StepError::Kernel(failure));
            }
        };
        self.update_owner(key.duty(), turn, false);
        self.finish_action(meta, turn, observations)
    }

    fn execute_delivery(
        &mut self,
        key: ActionKey,
        id: ActionId,
        token: crate::EventToken,
    ) -> Result<ActionRecord<M::Observation>, StepError<M::Error, N::Error, S::Error>> {
        let Some(routed) = self.timeline.cancel(token) else {
            self.fail();
            return Err(StepError::Kernel(KernelFailure::TimelineLostEvent(token)));
        };
        if routed.target != key.duty() {
            let actual = routed.target;
            self.fail();
            return Err(StepError::Kernel(KernelFailure::DeliveryTargetMismatch {
                token,
                expected: key.duty(),
                actual,
            }));
        }

        let now = self.now();
        let meta = ActionMeta::new(id, key, now, routed.cause);
        let delivery = Delivery::new(token, routed.event);
        let mut context = ActionContext::new(
            &mut self.timeline,
            &self.topology,
            &self.stopped,
            id,
            now,
            self.limits.action_context(),
        );
        let guard = PoisonGuard::new(&self.poisoned);
        let result = self
            .model
            .deliver(key.duty(), delivery, &mut context);
        guard.disarm();
        let turn = match result {
            Ok(turn) => turn,
            Err(source) => {
                drop(context);
                self.fail();
                return Err(StepError::Model {
                    action: meta,
                    source,
                });
            }
        };
        let observations = match commit_context(key.duty(), turn, context) {
            Ok(observations) => observations,
            Err(failure) => {
                self.fail();
                return Err(StepError::Kernel(failure));
            }
        };
        self.update_owner(key.duty(), turn, true);
        self.finish_action(meta, turn, observations)
    }

    fn update_owner(&mut self, duty: crate::DutyId, turn: Turn, delivery: bool) {
        let state = self
            .states
            .get_mut(&duty)
            .unwrap_or_else(|| panic!("selected duty must belong to topology"));
        state.next = turn.next();
        if delivery {
            state.deliveries = state.deliveries.saturating_add(1);
        } else {
            state.turns = state.turns.saturating_add(1);
        }
        if turn.next() == Next::Stop {
            self.stopped.insert(duty);
        }
    }

    fn finish_action(
        &mut self,
        meta: ActionMeta,
        turn: Turn,
        observations: Vec<M::Observation>,
    ) -> Result<ActionRecord<M::Observation>, StepError<M::Error, N::Error, S::Error>> {
        let record = ActionRecord::new(meta, turn, observations);
        let snapshot = self.snapshot();
        let view = SimulationView::new(
            &self.model,
            &self.topology,
            &self.states,
            snapshot,
        );
        let guard = PoisonGuard::new(&self.poisoned);
        let result = self.monitor.after_action(view, &record);
        guard.disarm();
        if let Err(source) = result {
            self.fail();
            return Err(StepError::Monitor {
                action: meta,
                source,
            });
        }
        Ok(record)
    }
}

fn commit_context<E: Retained, O: Retained>(
    duty: crate::DutyId,
    turn: Turn,
    context: ActionContext<'_, E, O>,
) -> Result<Vec<O>, KernelFailure> {
    if turn.next() == Next::Stop {
        let pending = context.pending_for(duty);
        if pending != 0 {
            drop(context);
            return Err(KernelFailure::StoppedWithPending { duty, pending });
        }
    }
    Ok(context.commit())
}
