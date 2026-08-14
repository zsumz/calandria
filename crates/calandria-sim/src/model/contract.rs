//! Consumer-owned dispatch across one or many production-shaped duties.

use calandria::{Moment, Retained, Turn};

use crate::{ActionContext, Delivery};

use super::DutyId;

/// Adapter that exposes concrete owners to deterministic execution.
///
/// The model does not replace [`calandria::Duty`]. Implementations normally
/// match a [`DutyId`] to a concrete owner and invoke that owner's production
/// `Duty::turn` implementation. Event delivery remains separate so a consumer
/// can preserve its real mailbox, readiness, or completion boundary.
pub trait Model {
    /// Typed delivery exchanged between modeled capability and duty owners.
    type Event: Retained;
    /// Structured observation emitted by successful actions.
    type Observation: Retained;
    /// Domain failure from one bounded model action.
    type Error;

    /// Executes one bounded owner turn.
    fn turn(
        &mut self,
        duty: DutyId,
        now: Moment,
        context: &mut ActionContext<'_, Self::Event, Self::Observation>,
    ) -> Result<Turn, Self::Error>;

    /// Delivers one owned modeled event to its target owner.
    fn deliver(
        &mut self,
        duty: DutyId,
        delivery: Delivery<Self::Event>,
        context: &mut ActionContext<'_, Self::Event, Self::Observation>,
    ) -> Result<Turn, Self::Error>;
}
