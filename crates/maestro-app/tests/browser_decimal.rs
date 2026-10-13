//! Shared decimal cleanup through the browser application interface.
#![cfg(test)]

#[test]
fn decimal_cleanup_preserves_integer_and_exponent_digits() {
    for (input, expected) in [
        ("1.2300", "1.23"),
        ("100.000", "100"),
        ("-0.000", "-0"),
        ("100", "100"),
        ("1.200e+10", "1.200e+10"),
        ("1.200E-10", "1.200E-10"),
    ] {
        assert_eq!(maestro_app::browser::strip_decimal(input), expected);
    }
}
