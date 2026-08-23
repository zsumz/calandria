//! Public finite-plan ordering, delay, and ownership contracts.

use calandria::Span;
use calandria_sim::{Plan, Planned};

#[test]
fn planned_values_and_finite_plans_preserve_order_delay_and_ownership() {
    let planned = Planned::new(Span::from_nanos(3), String::from("first"));
    assert_eq!(planned.delay(), Span::from_nanos(3));
    assert_eq!(planned.outcome(), "first");
    let mapped = planned.map(|value| value.len());
    assert_eq!(mapped, Planned::new(Span::from_nanos(3), 5));
    assert_eq!(mapped.into_outcome(), 5);

    let outcomes = [
        Planned::new(Span::from_nanos(2), 20),
        Planned::new(Span::from_nanos(1), 10),
    ];
    let plan: Plan<_> = outcomes.clone().into_iter().collect();
    assert_eq!(plan.len(), 2);
    assert!(!plan.is_empty());
    assert_eq!(plan.outcomes(), outcomes);
    assert_eq!(plan.into_outcomes(), outcomes);

    let empty = Plan::<u8>::empty();
    assert!(empty.is_empty());
    assert!(Plan::<u8>::default().is_empty());
    let single = Plan::single(Planned::new(Span::ZERO, 7));
    assert_eq!(single.into_outcomes(), [Planned::new(Span::ZERO, 7)]);
}
