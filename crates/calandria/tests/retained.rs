//! Fixed-width retained-byte arithmetic and conversion contracts.

use std::error::Error;

use calandria::{RetainedBytes, RetainedBytesOverflow};

#[test]
fn retained_byte_arithmetic_is_checked_and_fixed_width() {
    let three = RetainedBytes::new(3);
    let five = RetainedBytes::new(5);

    assert_eq!(three.checked_add(five), Some(RetainedBytes::new(8)));
    assert_eq!(five.checked_sub(three), Some(RetainedBytes::new(2)));
    assert_eq!(three.checked_sub(five), None);
    assert_eq!(RetainedBytes::new(u64::MAX).checked_add(three), None);
    assert_eq!(RetainedBytes::from(7_u32), RetainedBytes::new(7));
    assert_eq!(RetainedBytes::from(9_u64), RetainedBytes::new(9));
    assert_eq!(
        RetainedBytes::try_from(11_usize),
        Ok(RetainedBytes::new(11))
    );
}

#[test]
fn retained_byte_conversion_error_has_a_stable_public_diagnostic() {
    let error = RetainedBytesOverflow;

    assert_eq!(
        error.to_string(),
        "retained byte count exceeds the u64 accounting domain"
    );
    assert!(Error::source(&error).is_none());
}
