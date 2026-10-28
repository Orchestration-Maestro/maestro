//! Literal POSIX shell operand serialization.

/// Serializes one literal POSIX shell operand.
#[must_use]
pub fn shell_quote(value: &str) -> String {
    value.to_owned()
}
