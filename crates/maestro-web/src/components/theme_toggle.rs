//! Light, dark and system appearance for the page's brand pack.

/// The page storage key holding the chosen appearance.
pub const THEME_KEY: &str = "theme";

/// The appearance shown for a stored value: the value itself, or `system` when absent or empty.
#[must_use]
pub fn initial_theme(stored: Option<&str>) -> &str {
    stored.filter(|theme| !theme.is_empty()).unwrap_or("system")
}
/// The appearance after `theme` in the cycle: light then dark, then system when it is included.
///
/// Without system in the cycle, system counts as light; an unknown value starts the cycle again.
#[must_use]
pub fn next_theme(theme: &str, include_system: bool) -> &'static str {
    let cycle: &[&'static str] = if include_system {
        &["light", "dark", "system"]
    } else {
        &["light", "dark"]
    };
    let current = if !include_system && theme == "system" {
        "light"
    } else {
        theme
    };
    cycle
        .iter()
        .position(|candidate| *candidate == current)
        .map_or(cycle[0], |index| cycle[(index + 1) % cycle.len()])
}
/// Whether `theme` shows the dark mode, following the platform for `system`.
#[must_use]
pub fn is_dark(theme: &str, system_dark: bool) -> bool {
    match theme {
        "dark" => true,
        "system" => system_dark,
        _ => false,
    }
}
/// The catalog icon for `theme`: the sun for light and any unknown value, which shows light.
#[must_use]
pub fn theme_icon(theme: &str) -> &'static str {
    match theme {
        "dark" => "Moon",
        "system" => "Monitor",
        _ => "Sun",
    }
}

#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(target_arch = "wasm32")]
pub use browser::ThemeToggle;

#[cfg(test)]
mod tests;
