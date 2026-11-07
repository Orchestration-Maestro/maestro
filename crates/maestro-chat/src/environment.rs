//! Process environment reads shared by the presentation.

/// Reads one environment variable as text, substituting replacement characters for bytes
/// that are not UTF-8.
pub(crate) fn variable(name: &str) -> Option<String> {
    std::env::var_os(name).map(|value| value.to_string_lossy().into_owned())
}
