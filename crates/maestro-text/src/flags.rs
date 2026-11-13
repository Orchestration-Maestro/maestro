//! Environment reads and flag truth.

/// Read one environment variable, replacing bytes that are not UTF-8 with U+FFFD.
#[must_use]
pub fn environment_variable(name: &str) -> Option<String> {
    std::env::var_os(name).map(|value| value.to_string_lossy().into_owned())
}

/// Accept `1`, ASCII-case-insensitive `true` or `yes`, without trimming.
#[must_use]
pub fn is_truthy_env_flag(value: Option<&str>) -> bool {
    value.is_some_and(|value| {
        value == "1" || value.eq_ignore_ascii_case("true") || value.eq_ignore_ascii_case("yes")
    })
}

/// Whether an offline selection, or `MAESTRO_OFFLINE` when the selection is absent,
/// disables remote work.
#[must_use]
pub fn is_offline_selected(selection: Option<bool>) -> bool {
    selection
        .unwrap_or_else(|| is_truthy_env_flag(std::env::var("MAESTRO_OFFLINE").ok().as_deref()))
}
