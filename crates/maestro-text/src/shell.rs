//! Literal POSIX shell operand serialization.

/// Serializes one literal POSIX shell operand.
#[must_use]
pub fn shell_quote(value: &str) -> String {
    if !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"@%_+=:,./-".contains(&byte))
    {
        value.to_owned()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}
