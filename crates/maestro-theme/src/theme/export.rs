//! Resolved colors for export: palette expansion, ordered records and optional fields.
use super::loading::resolve;
use super::{ColorValue, ThemeError, ThemeState};
use serde_json::Value;

/// Explicit export colors of a theme; an unset or empty field is absent.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ThemeExportColors {
    /// Page background.
    pub page_bg: Option<String>,
    /// Card background.
    pub card_bg: Option<String>,
    /// Info background.
    pub info_bg: Option<String>,
}

/// Report whether the supplied name is exactly `light`.
#[must_use]
pub fn is_light_theme(name: Option<&str>) -> bool {
    name == Some("light")
}

impl ThemeState {
    /// Resolve every color of a theme to a CSS hex string, in canonical key order.
    ///
    /// The name is the argument, else the selected theme, else the terminal
    /// background's theme. Empty colors become black for `light`, otherwise `#e5e5e7`.
    ///
    /// # Errors
    /// Returns document selection, parsing, admission and alias failures.
    pub fn get_resolved_theme_colors(
        &self,
        name: Option<&str>,
    ) -> Result<Vec<(String, String)>, ThemeError> {
        let name = self.export_name(name);
        let default_text = if is_light_theme(Some(&name)) {
            "#000000"
        } else {
            "#e5e5e7"
        };
        let document = self.document(&name)?;
        let colors = document["colors"]
            .as_object()
            .ok_or_else(|| ThemeError::message("Theme colors must be an object".to_owned()))?;
        let mut entries: Vec<_> = colors.iter().collect();
        entries.sort_by_key(|(key, _)| array_index(key).unwrap_or(u32::MAX));
        entries
            .into_iter()
            .map(|(key, value)| {
                let color = match resolve(value, &document["vars"])? {
                    ColorValue::Index(index) => ansi256_hex(index),
                    ColorValue::String(color) if color.is_empty() => default_text.to_owned(),
                    ColorValue::String(color) => color,
                };
                Ok((key.clone(), color))
            })
            .collect()
    }

    /// Resolve the explicit export colors of a theme; any failure gives no colors.
    #[must_use]
    pub fn get_theme_export_colors(&self, name: Option<&str>) -> ThemeExportColors {
        self.export_colors(name).unwrap_or_default()
    }

    /// Select the argument, else the selected name, else the default theme.
    fn export_name(&self, name: Option<&str>) -> String {
        name.map(str::to_owned)
            .or_else(|| self.lifecycle.name.borrow().clone())
            .unwrap_or_else(|| self.default_theme())
    }

    /// Resolve each export field after the one before it.
    fn export_colors(&self, name: Option<&str>) -> Result<ThemeExportColors, ThemeError> {
        let document = self.document(&self.export_name(name))?;
        let Some(section) = document.get("export") else {
            return Ok(ThemeExportColors::default());
        };
        let field = |key: &str| export_color(section.get(key), &document["vars"]);
        Ok(ThemeExportColors {
            page_bg: field("pageBg")?,
            card_bg: field("cardBg")?,
            info_bg: field("infoBg")?,
        })
    }
}

/// Resolve one optional export field; an empty color is absent.
fn export_color(value: Option<&Value>, vars: &Value) -> Result<Option<String>, ThemeError> {
    let Some(value) = value else {
        return Ok(None);
    };
    Ok(match resolve(value, vars)? {
        ColorValue::Index(index) => Some(ansi256_hex(index)),
        ColorValue::String(color) if color.is_empty() => None,
        ColorValue::String(color) => Some(color),
    })
}

/// Parse a canonical array index: no sign, no leading zero, below 2^32 - 1.
fn array_index(key: &str) -> Option<u32> {
    let canonical =
        key == "0" || (!key.starts_with('0') && key.bytes().all(|b| b.is_ascii_digit()));
    canonical
        .then(|| key.parse().ok())
        .flatten()
        .filter(|index| *index < u32::MAX)
}

/// Basic palette entries, which terminals render approximately.
const BASIC: [&str; 16] = [
    "#000000", "#800000", "#008000", "#808000", "#000080", "#800080", "#008080", "#c0c0c0",
    "#808080", "#ff0000", "#00ff00", "#ffff00", "#0000ff", "#ff00ff", "#00ffff", "#ffffff",
];

/// Expand a palette index to a hex color.
fn ansi256_hex(index: u8) -> String {
    match index {
        0..=15 => BASIC[usize::from(index)].to_owned(),
        16..=231 => {
            let cube = index - 16;
            let level = |n: u8| if n == 0 { 0 } else { 55 + n * 40 };
            let [r, g, b] = [cube / 36, cube % 36 / 6, cube % 6].map(level);
            format!("#{r:02x}{g:02x}{b:02x}")
        }
        _ => {
            let gray = 8 + (index - 232) * 10;
            format!("#{gray:02x}{gray:02x}{gray:02x}")
        }
    }
}
