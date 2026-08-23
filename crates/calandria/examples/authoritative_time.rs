//! Deterministic timeout decisions from one authoritative duty moment.

use core::convert::Infallible;

use calandria::{Deadline, Duty, Moment, Turn, WorkCount};

struct Timeout {
    deadline: Deadline,
}

impl Duty for Timeout {
    type Error = Infallible;

    fn turn(&mut self, now: Moment) -> Result<Turn, Self::Error> {
        if self.deadline.is_elapsed_at(now) {
            Ok(Turn::stopped(WorkCount::new(1)))
        } else {
            Ok(Turn::until(WorkCount::ZERO, self.deadline))
        }
    }
}

fn main() {
    let deadline = Deadline::at(Moment::from_nanos(20));
    let supplied = Moment::from_nanos(10);
    let mut first = Timeout { deadline };
    let mut replay = Timeout { deadline };

    assert_eq!(first.turn(supplied), replay.turn(supplied));
    assert_eq!(
        first.turn(supplied),
        Ok(Turn::until(WorkCount::ZERO, deadline))
    );
    assert_eq!(
        first.turn(Moment::from_nanos(20)),
        Ok(Turn::stopped(WorkCount::new(1)))
    );
}
