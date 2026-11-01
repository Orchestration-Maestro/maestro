//! The key names a recorded corpus spells for the platform it was recorded on.

/// The recorded text with the alt key shown under the platform's name for it; the corpus
/// spells it `Alt`.
pub fn on_this_platform(recorded: &str) -> String {
    if cfg!(target_os = "macos") {
        recorded.replace("Alt", "Option")
    } else {
        recorded.to_owned()
    }
}
