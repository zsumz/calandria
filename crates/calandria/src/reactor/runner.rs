//! Bounded turn-and-wait loop for one owned reactor.

use crate::{Clock, Duty, EmbeddedHost, HostAction, Waiter};

use super::{
    ReactorExit, ReactorFailure, ReactorOutcome, ReactorSnapshot, control::ReactorControlOwner,
};

pub(super) fn run<D, C, W>(
    mut host: EmbeddedHost<D, C>,
    mut waiter: W,
    control: &ReactorControlOwner,
) -> ReactorExit<D, C, W>
where
    D: Duty,
    C: Clock,
    W: Waiter<D>,
{
    let mut snapshot = ReactorSnapshot::default();
    loop {
        if control.termination_requested() {
            host.terminate();
            let _ = control.finish();
            return ReactorExit {
                host,
                waiter,
                outcome: ReactorOutcome::Terminated,
                snapshot,
            };
        }

        let step = match host.step() {
            Ok(step) => step,
            Err(source) => {
                let _ = control.finish();
                return ReactorExit {
                    host,
                    waiter,
                    outcome: ReactorOutcome::Failed(ReactorFailure::Host(source)),
                    snapshot,
                };
            }
        };

        match step.action() {
            HostAction::Continue => {}
            HostAction::Stop => {
                let outcome = if control.finish() {
                    host.terminate();
                    ReactorOutcome::Terminated
                } else {
                    ReactorOutcome::Stopped
                };
                return ReactorExit {
                    host,
                    waiter,
                    outcome,
                    snapshot,
                };
            }
            HostAction::Wait(maximum) => {
                if control.termination_requested() {
                    continue;
                }
                match waiter.wait(host.duty_mut(), maximum) {
                    Ok(outcome) => snapshot.record(outcome),
                    Err(source) => {
                        host.fail();
                        let _ = control.finish();
                        return ReactorExit {
                            host,
                            waiter,
                            outcome: ReactorOutcome::Failed(ReactorFailure::Wait(source)),
                            snapshot,
                        };
                    }
                }
            }
        }
    }
}
