//! Deterministic shortening with wrapping UTF-16 mixing.

/// Shorten text to the concatenated unsigned base-36 hash words.
#[must_use]
pub fn short_hash(value: &str) -> String {
    let (mut first, mut second) = (0xdead_beef_u32, 0x41c6_ce57_u32);
    for unit in value.encode_utf16() {
        first = (first ^ u32::from(unit)).wrapping_mul(2_654_435_761);
        second = (second ^ u32::from(unit)).wrapping_mul(1_597_334_677);
    }
    first = (first ^ (first >> 16)).wrapping_mul(2_246_822_507)
        ^ (second ^ (second >> 13)).wrapping_mul(3_266_489_909);
    second = (second ^ (second >> 16)).wrapping_mul(2_246_822_507)
        ^ (first ^ (first >> 13)).wrapping_mul(3_266_489_909);
    base36(second) + &base36(first)
}

fn base36(mut value: u32) -> String {
    let mut digits = Vec::new();
    loop {
        if let Some(digit) = char::from_digit(value % 36, 36) {
            digits.push(digit);
        }
        value /= 36;
        if value == 0 {
            return digits.into_iter().rev().collect();
        }
    }
}
