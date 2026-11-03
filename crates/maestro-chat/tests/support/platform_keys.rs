//! Platform-specific recorded expectations and key names.

/// Selects the complete macOS expectation when supplied on macOS.
pub fn for_this_platform<T>(expected: T, macos: Option<T>) -> T {
    if cfg!(target_os = "macos") {
        macos.unwrap_or(expected)
    } else {
        expected
    }
}

/// The recorded text with the alt key shown under the platform's name for it; the corpus
/// spells it `Alt`.
pub fn on_this_platform(recorded: &str) -> String {
    if cfg!(target_os = "macos") {
        recorded.replace("Alt", "Option")
    } else {
        recorded.to_owned()
    }
}
