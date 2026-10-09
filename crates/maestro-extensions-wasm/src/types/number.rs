//! The codec of a floating-point property: finite values are JSON numbers, nonfinite values
//! spell their IEEE-754 bits, which JSON cannot carry.

use std::borrow::Borrow;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serializer};

/// Prefix of the text that stands for a nonfinite number.
const PREFIX: &str = "f64:";

/// The number digits of a nonfinite value must be exactly this many hexadecimal digits.
const DIGITS: usize = 16;

/// What a number property holds on the wire: a JSON number or the text of a nonfinite value.
#[derive(Deserialize)]
#[serde(untagged)]
enum Wire {
    /// An ordinary JSON number.
    Number(f64),
    /// The spelling of a nonfinite value.
    Bits(String),
}

/// Writes a finite value as a JSON number and any other value as `f64:` followed by its 16
/// lowercase hexadecimal IEEE-754 digits. The value is any borrow of a number because serde
/// hands over a reference, which Clippy would have a plain `f64` take by value.
///
/// # Errors
/// Returns the serializer's error.
pub(crate) fn serialize<S: Serializer>(
    value: &impl Borrow<f64>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let value = *value.borrow();
    if value.is_finite() {
        serializer.serialize_f64(value)
    } else {
        serializer.collect_str(&format_args!("{PREFIX}{:016x}", value.to_bits()))
    }
}

/// Reads a JSON number, or the spelling of a nonfinite value.
///
/// # Errors
/// Returns an error for any other JSON value, for text that is not that spelling, and for a
/// spelling whose bits are finite.
pub(crate) fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
    match Wire::deserialize(deserializer)? {
        Wire::Number(value) => Ok(value),
        Wire::Bits(text) => {
            nonfinite(&text).ok_or_else(|| D::Error::custom("invalid nonfinite number encoding"))
        }
    }
}

/// The nonfinite value a spelling stands for.
fn nonfinite(text: &str) -> Option<f64> {
    let digits = text.strip_prefix(PREFIX)?;
    if digits.len() != DIGITS || !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let value = f64::from_bits(u64::from_str_radix(digits, 16).ok()?);
    (!value.is_finite()).then_some(value)
}
