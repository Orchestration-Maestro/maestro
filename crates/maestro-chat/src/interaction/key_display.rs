//! Displayed key names.
use super::InteractiveMode;

impl InteractiveMode {
    /// Capitalizes the first character of every `+`-separated part of each `/`-separated
    /// key, for example `ctrl+c/alt+enter` as `Ctrl+C/Alt+Enter`.
    pub(super) fn capitalize_key(key: &str) -> String {
        key.split('/')
            .map(|binding| {
                let parts: Vec<String> = binding.split('+').map(capitalize_first).collect();
                parts.join("+")
            })
            .collect::<Vec<_>>()
            .join("/")
    }

    /// The capitalized keys currently bound to the application `action`.
    pub(super) fn get_app_key_display(action: &str) -> String {
        Self::capitalize_key(&maestro_app::key_text(action))
    }
}

/// `part` with its first character in upper case.
fn capitalize_first(part: &str) -> String {
    let mut chars = part.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}
