//! Gated reactor worker creation and terminal publication.

use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, Mutex, MutexGuard, mpsc::SyncSender},
    thread::{self, JoinHandle, Thread},
};

use crate::{Clock, Duty, Reactor, ReactorOutcome, Waiter};

use super::{GroupControl, ReactorGroupMemberExit, ReactorId, StartDecision, StartGate};

pub(super) type ReactorSlot<D, C, W> = Arc<Mutex<Option<Reactor<D, C, W>>>>;

pub(super) fn reactor_slot<D, C, W>(reactor: Reactor<D, C, W>) -> ReactorSlot<D, C, W> {
    Arc::new(Mutex::new(Some(reactor)))
}

pub(super) enum WorkerResult<D, C, W>
where
    D: Duty,
    C: Clock,
    W: Waiter<D>,
{
    Aborted,
    Finished(ReactorGroupMemberExit<D, C, W>),
}

pub(super) struct WorkerJoin<D, C, W>
where
    D: Duty,
    C: Clock,
    W: Waiter<D>,
{
    join: JoinHandle<WorkerResult<D, C, W>>,
}

impl<D, C, W> WorkerJoin<D, C, W>
where
    D: Duty,
    C: Clock,
    W: Waiter<D>,
{
    pub(super) fn thread(&self) -> &Thread {
        self.join.thread()
    }

    pub(super) fn join(self) -> thread::Result<WorkerResult<D, C, W>> {
        self.join.join()
    }
}

pub(super) fn spawn_worker<D, C, W, T>(
    id: ReactorId,
    name: String,
    slot: ReactorSlot<D, C, W>,
    gate: StartGate,
    completed: SyncSender<ReactorId>,
    group: GroupControl<T>,
) -> Result<WorkerJoin<D, C, W>, std::io::Error>
where
    D: Duty + Send + 'static,
    D::Error: Send + 'static,
    C: Clock + Send + 'static,
    C::Error: Send + 'static,
    W: Waiter<D> + Send + 'static,
    W::Error: Send + 'static,
    T: Send + 'static,
{
    if name.as_bytes().contains(&0) {
        return Err(super::super::handle::invalid_thread_name());
    }
    let join = thread::Builder::new().name(name).spawn(move || {
        if gate.wait() == StartDecision::Abort {
            return WorkerResult::Aborted;
        }
        let _completion = Completion { id, completed };
        let reactor = take_reactor(&slot);
        let exit = catch_unwind(AssertUnwindSafe(|| reactor.run()));
        let exit = match exit {
            Ok(exit) => ReactorGroupMemberExit::Exited(exit),
            Err(payload) => ReactorGroupMemberExit::Panicked(payload),
        };
        if is_fatal(&exit) {
            drop(group.request_termination());
        }
        WorkerResult::Finished(exit)
    })?;
    Ok(WorkerJoin { join })
}

struct Completion {
    id: ReactorId,
    completed: SyncSender<ReactorId>,
}

impl Drop for Completion {
    fn drop(&mut self) {
        let _ = self.completed.send(self.id);
    }
}

pub(super) fn take_reactor<D, C, W>(slot: &ReactorSlot<D, C, W>) -> Reactor<D, C, W> {
    lock(slot)
        .take()
        .unwrap_or_else(|| panic!("reactor group ownership slot was already consumed"))
}

fn is_fatal<D, C, W>(exit: &ReactorGroupMemberExit<D, C, W>) -> bool
where
    D: Duty,
    C: Clock,
    W: Waiter<D>,
{
    matches!(
        exit,
        ReactorGroupMemberExit::Exited(exit)
            if matches!(exit.outcome(), ReactorOutcome::Failed(_))
    ) || matches!(exit, ReactorGroupMemberExit::Panicked(_))
}

fn lock<T>(slot: &Mutex<T>) -> MutexGuard<'_, T> {
    slot.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
