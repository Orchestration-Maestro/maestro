//! Author-written number prefixes and half-up rounding.

use maestro_text::numeric::{decimal_integer_prefix, integer_prefix, round};

#[test]
fn a_decimal_prefix_reads_its_leading_digits() {
    assert_eq!(
        decimal_integer_prefix(" 12px").to_bits(),
        12.0_f64.to_bits()
    );
    assert_eq!(
        decimal_integer_prefix("-7 of 9").to_bits(),
        (-7.0_f64).to_bits()
    );
    assert_eq!(decimal_integer_prefix("+3").to_bits(), 3.0_f64.to_bits());
    assert!(decimal_integer_prefix("px").is_nan());
}

#[test]
fn a_radix_prefix_reads_hexadecimal_notation_only_when_it_admits_it() {
    assert_eq!(integer_prefix("0x1f", 10).to_bits(), 31.0_f64.to_bits());
    assert_eq!(integer_prefix("101", 2).to_bits(), 5.0_f64.to_bits());
}

#[test]
fn rounding_moves_an_exact_half_toward_positive_infinity() {
    assert_eq!(round(2.5).to_bits(), 3.0_f64.to_bits());
    assert_eq!(round(-2.5).to_bits(), (-2.0_f64).to_bits());
}

#[test]
fn rounding_a_small_negative_keeps_the_negative_zero() {
    assert!(round(-0.2).is_sign_negative());
}
