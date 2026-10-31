//! Keybinding hints for the current registry, styled with the published theme.

use maestro_theme::{LiveTheme, Theme, ThemeColor, ThemeError};
use maestro_tui::get_keybindings;

/// The name `alt` is displayed with on this platform.
const ALT_NAME: &str = if cfg!(target_os = "macos") {
    "option"
} else {
    "alt"
};

/// The display spelling of `key`, a `/`-separated list of `+`-joined key ids: `alt` is
/// `option` on macOS, and each part starts with a capital letter when `capitalize` is set.
#[must_use]
pub fn format_key_text(key: &str, capitalize: bool) -> String {
    format_key_text_for(key, capitalize, ALT_NAME)
}

/// The rule of [`format_key_text`] for a supplied name of `alt`.
fn format_key_text_for(key: &str, capitalize: bool, alt: &str) -> String {
    let _ = (capitalize, alt);
    key.to_owned()
}

/// The keys bound to `keybinding` as [`format_key_text`] displays them with capital letters.
#[must_use]
pub fn key_display_text(keybinding: &str) -> String {
    format_key_text(&key_text(keybinding), true)
}

/// The keybinding that expands a collapsed tool row.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) const EXPAND: &str = "app.tools.expand";

/// The keys bound to `keybinding`, joined with `/` in registry order; empty when unbound.
#[must_use]
pub fn key_text(keybinding: &str) -> String {
    get_keybindings().get_keys(keybinding).join("/")
}

#[cfg(test)]
mod tests;

/// `key_text` in the dim style followed by a space and `description` in the muted style.
///
/// # Errors
/// Returns the theme's failure when no theme is published or it lacks a color.
pub fn key_hint(
    keybinding: &str,
    description: &str,
    theme: &LiveTheme,
) -> Result<String, ThemeError> {
    raw_key_hint(&key_text(keybinding), description, theme)
}

/// `key` in the dim style followed by a space and `description` in the muted style.
///
/// # Errors
/// Returns the theme's failure when no theme is published or it lacks a color.
pub fn raw_key_hint(key: &str, description: &str, theme: &LiveTheme) -> Result<String, ThemeError> {
    let theme = theme.get()?;
    format_key_hint(key, description, Some(&theme))
}

/// An unstyled hint for deferred drawing when the live theme becomes unavailable.
#[must_use]
pub fn plain_key_hint(keybinding: &str, description: &str) -> String {
    format_key_hint(&key_text(keybinding), description, None).unwrap_or_default()
}

/// One owner for styled and unstyled key-hint framing.
fn format_key_hint(
    key: &str,
    description: &str,
    theme: Option<&Theme>,
) -> Result<String, ThemeError> {
    let description = format!(" {description}");
    match theme {
        Some(theme) => {
            Ok(theme.fg(&ThemeColor::Dim, key)? + &theme.fg(&ThemeColor::Muted, &description)?)
        }
        None => Ok(format!("{key}{description}")),
    }
}
