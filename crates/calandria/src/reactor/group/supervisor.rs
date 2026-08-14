//! Bounded completion supervision for a static reactor group.

use std::{
    sync::{Arc, Mutex, MutexGuard, mpsc::Receiver},
    thread::{self, JoinHandle},
};

use crate::{Clock, Duty, ReactorOutcome, Waiter};

use super::{
    AdmissionCloser, ReactorGroupMemberExit, ReactorGroupOutcome, Supervised, WorkerJoin,
    WorkerResult, exit::terminal_kind,
};

type SupervisorSpawn<D, C, W, T> =
    Result<JoinHandle<Supervised<D, C, W>>, (std::io::Error, SupervisorAssets<D, C, W, T>)>;

pub(super) struct SupervisorAssets<D, C, W, T>
where
    D: Duty,
    C: Clock,
    W: Waiter<D>,
{
    workers: Vec<WorkerJoin<D, C, W>>,
    completed: Receiver<super::ReactorId>,
    admission: AdmissionCloser<T>,
}

impl<D, C, W, T> SupervisorAssets<D, C, W, T>
where
    D: Duty,
    C: Clock,
    W: Waiter<D>,
{
    pub(super) const fn new(
        workers: Vec<WorkerJoin<D, C, W>>,
        completed: Receiver<super::ReactorId>,
        admission: AdmissionCloser<T>,
    ) -> Self {
        Self {
            workers,
            completed,
            admission,
        }
    }

    pub(super) fn into_workers(self) -> Vec<WorkerJoin<D, C, W>> {
        self.workers
    }
}

pub(super) fn spawn_supervisor<D, C, W, T>(
    name: String,
    assets: SupervisorAssets<D, C, W, T>,
) -> SupervisorSpawn<D, C, W, T>
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
        return Err((super::super::handle::invalid_thread_name(), assets));
    }
    let slot = Arc::new(Mutex::new(Some(assets)));
    let thread_slot = Arc::clone(&slot);
    match thread::Builder::new()
        .name(name)
        .spawn(move || supervise(take(&thread_slot)))
    {
        Ok(join) => Ok(join),
        Err(source) => Err((source, take(&slot))),
    }
}

fn supervise<D, C, W, T>(assets: SupervisorAssets<D, C, W, T>) -> Supervised<D, C, W>
where
    D: Duty,
    C: Clock,
    W: Waiter<D>,
{
    let count = assets.workers.len();
    let mut workers = assets.workers.into_iter().map(Some).collect::<Vec<_>>();
    let mut exits = core::iter::repeat_with(|| None)
        .take(count)
        .collect::<Vec<Option<ReactorGroupMemberExit<D, C, W>>>>();
    let mut fatal = None;

    for _ in 0..count {
        let id = assets
            .completed
            .recv()
            .unwrap_or_else(|_| panic!("reactor group completion channel closed early"));
        let position = id
            .position()
            .unwrap_or_else(|| panic!("reactor group published an unrepresentable identity"));
        let worker = workers
            .get_mut(position)
            .and_then(Option::take)
            .unwrap_or_else(|| panic!("reactor group published duplicate completion"));
        let result = worker.join().unwrap_or_else(|payload| {
            WorkerResult::Finished(ReactorGroupMemberExit::Panicked(payload))
        });
        let WorkerResult::Finished(exit) = result else {
            panic!("started reactor group member reported startup abort");
        };
        if fatal.is_none() {
            fatal = terminal_kind(id, &exit);
        }
        exits[position] = Some(exit);
    }

    assets.admission.close();
    let members = exits
        .into_iter()
        .map(|exit| exit.unwrap_or_else(|| panic!("reactor group terminal exit missing")))
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let outcome = fatal.unwrap_or_else(|| aggregate(&members));
    Supervised { outcome, members }
}

fn aggregate<D, C, W>(members: &[ReactorGroupMemberExit<D, C, W>]) -> ReactorGroupOutcome
where
    D: Duty,
    C: Clock,
    W: Waiter<D>,
{
    if members.iter().any(|member| {
        matches!(
            member,
            ReactorGroupMemberExit::Exited(exit)
                if matches!(exit.outcome(), ReactorOutcome::Terminated)
        )
    }) {
        ReactorGroupOutcome::Terminated
    } else {
        ReactorGroupOutcome::Stopped
    }
}

fn take<T>(slot: &Mutex<Option<T>>) -> T {
    lock(slot)
        .take()
        .unwrap_or_else(|| panic!("reactor group supervisor assets were already consumed"))
}

fn lock<T>(slot: &Mutex<T>) -> MutexGuard<'_, T> {
    slot.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
