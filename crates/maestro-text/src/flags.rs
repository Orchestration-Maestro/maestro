//! Environment flag truth.

/// Accept `1`, ASCII-case-insensitive `true` or `yes`, without trimming.
#[must_use]
pub fn is_truthy_env_flag(value: Option<&str>) -> bool {
    value.is_some_and(|value| {
        value == "1" || value.eq_ignore_ascii_case("true") || value.eq_ignore_ascii_case("yes")
    })
}
