//! Ownership-safe static reactor-group validation and startup.

use std::{num::NonZeroUsize, sync::mpsc, thread::Thread};

use crate::{Clock, Duty, Reactor, Waiter};

use super::{
    GroupControl, ReactorGroup, ReactorGroupLimits, ReactorGroupMember, ReactorGroupSpawnError,
    ReactorGroupSpawnFailure, ReactorId, ReactorSlot, StartGate, SupervisorAssets, WorkerJoin,
    WorkerResult, bounded_ingress, reactor_slot, spawn_supervisor, spawn_worker,
    worker::take_reactor,
};

impl<D, C, W, T> ReactorGroup<D, C, W, T>
where
    D: Duty + Send + 'static,
    D::Error: Send + 'static,
    C: Clock + Send + 'static,
    C::Error: Send + 'static,
    W: Waiter<D> + Send + 'static,
    W::Error: Send + 'static,
    T: Send + 'static,
{
    /// Starts a bounded static topology under one fail-closed supervisor.
    pub fn spawn(
        limits: ReactorGroupLimits,
        name: impl Into<String>,
        members: Vec<ReactorGroupMember<D, C, W, T>>,
    ) -> Result<Self, ReactorGroupSpawnError<D, C, W, T>> {
        if let Err(failure) = validate(limits, members.len()) {
            return Err(ReactorGroupSpawnError::new(failure, members));
        }
        let reactors = NonZeroUsize::new(members.len())
            .unwrap_or_else(|| panic!("validated reactor group became empty"));
        let name = name.into();
        let (reactors_to_start, senders): (Vec<_>, Vec<_>) = members
            .into_iter()
            .map(ReactorGroupMember::into_parts)
            .unzip();
        let controls = reactors_to_start
            .iter()
            .map(Reactor::control)
            .collect::<Vec<_>>();
        let slots = reactors_to_start
            .into_iter()
            .map(reactor_slot)
            .collect::<Vec<_>>();
        let (ingress, closer) = bounded_ingress(senders);
        let control = GroupControl::new(controls, closer);
        let gate = StartGate::new();
        let (completed, completions) = mpsc::sync_channel(reactors.get());
        let mut pending = Pending {
            slots,
            ingress,
            control,
            gate,
            workers: Vec::with_capacity(reactors.get()),
            threads: Vec::with_capacity(reactors.get()),
        };

        for index in 0..reactors.get() {
            let id = ReactorId::from_position(index);
            let worker = spawn_worker(
                id,
                format!("{name}-{index}"),
                pending.slots[index].clone(),
                pending.gate.clone(),
                completed.clone(),
                pending.control.clone(),
            );
            match worker {
                Ok(worker) => pending.push(worker),
                Err(source) => {
                    drop(completed);
                    return Err(pending.abort(ReactorGroupSpawnFailure::ReactorThread {
                        reactor: id,
                        source,
                    }));
                }
            }
        }
        drop(completed);

        let assets = SupervisorAssets::new(
            core::mem::take(&mut pending.workers),
            completions,
            pending.control.admission(),
        );
        let supervisor = match spawn_supervisor(format!("{name}-supervisor"), assets) {
            Ok(supervisor) => supervisor,
            Err((source, assets)) => {
                pending.workers = assets.into_workers();
                return Err(pending.abort(ReactorGroupSpawnFailure::SupervisorThread { source }));
            }
        };

        pending.gate.start();
        Ok(Self {
            limits,
            reactors,
            ingress: pending.ingress,
            control: pending.control,
            threads: pending.threads.into_boxed_slice(),
            supervisor,
        })
    }
}

struct Pending<D, C, W, T>
where
    D: Duty,
    C: Clock,
    W: Waiter<D>,
{
    slots: Vec<ReactorSlot<D, C, W>>,
    ingress: super::ReactorGroupHandle<T>,
    control: GroupControl<T>,
    gate: StartGate,
    workers: Vec<WorkerJoin<D, C, W>>,
    threads: Vec<Thread>,
}

impl<D, C, W, T> Pending<D, C, W, T>
where
    D: Duty,
    C: Clock,
    W: Waiter<D>,
{
    fn push(&mut self, worker: WorkerJoin<D, C, W>) {
        self.threads.push(worker.thread().clone());
        self.workers.push(worker);
    }

    fn abort(self, failure: ReactorGroupSpawnFailure) -> ReactorGroupSpawnError<D, C, W, T> {
        self.gate.abort();
        for worker in self.workers {
            let result = worker
                .join()
                .unwrap_or_else(|_| panic!("aborted reactor worker panicked"));
            assert!(matches!(result, WorkerResult::Aborted));
        }
        drop(self.control);
        let reactors = self.slots.iter().map(take_reactor).collect::<Vec<_>>();
        let senders = self.ingress.into_senders();
        let members = reactors
            .into_iter()
            .zip(senders)
            .map(|(reactor, ingress)| ReactorGroupMember::from_parts(reactor, ingress))
            .collect();
        ReactorGroupSpawnError::new(failure, members)
    }
}

fn validate(limits: ReactorGroupLimits, actual: usize) -> Result<(), ReactorGroupSpawnFailure> {
    if actual == 0 {
        return Err(ReactorGroupSpawnFailure::Empty);
    }
    if actual > limits.reactors().get() {
        return Err(ReactorGroupSpawnFailure::Capacity {
            limit: limits.reactors(),
            actual,
        });
    }
    Ok(())
}
