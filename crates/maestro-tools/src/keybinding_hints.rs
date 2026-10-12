//! Keybinding hints for the current registry, styled with the published theme.

use maestro_theme::{LiveTheme, ThemeColor, ThemeError};
use maestro_tui::get_keybindings;

/// The keys bound to `keybinding`, joined with `/` in registry order; empty when unbound.
#[must_use]
pub fn key_text(keybinding: &str) -> String {
    get_keybindings().get_keys(keybinding).join("/")
}

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
    Ok(theme.fg(&ThemeColor::Dim, key)?
        + &theme.fg(&ThemeColor::Muted, &format!(" {description}"))?)
}
