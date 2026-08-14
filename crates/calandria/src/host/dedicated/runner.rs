//! Turn and wait loop owned by one dedicated thread.

use super::{DedicatedExit, DedicatedFailure, DedicatedOutcome, DedicatedSnapshot};
use crate::host::{Clock, Duty, EmbeddedHost, HostAction, Waiter};

pub(super) fn run<D, C, W>(mut host: EmbeddedHost<D, C>, mut waiter: W) -> DedicatedExit<D, C, W>
where
    D: Duty,
    C: Clock,
    W: Waiter<D>,
{
    let mut dedicated = DedicatedSnapshot::default();
    loop {
        let step = match host.step() {
            Ok(step) => step,
            Err(source) => {
                return DedicatedExit {
                    host,
                    waiter,
                    outcome: DedicatedOutcome::Failed(DedicatedFailure::Host(source)),
                    dedicated,
                };
            }
        };

        match step.action() {
            HostAction::Continue => {}
            HostAction::Stop => {
                return DedicatedExit {
                    host,
                    waiter,
                    outcome: DedicatedOutcome::Stopped,
                    dedicated,
                };
            }
            HostAction::Wait(maximum) => match waiter.wait(host.duty_mut(), maximum) {
                Ok(outcome) => dedicated.record(outcome),
                Err(source) => {
                    host.fail();
                    return DedicatedExit {
                        host,
                        waiter,
                        outcome: DedicatedOutcome::Failed(DedicatedFailure::Wait(source)),
                        dedicated,
                    };
                }
            },
        }
    }
}
