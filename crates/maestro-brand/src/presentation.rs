//! Resolution of selected palette, font, glyph and terminal aliases.
use crate::BrandError;
use crate::pack::{BrandFont, BrandMark, BrandType, PackData};
use std::collections::BTreeMap;

/// The two shipped appearances a pack supplies colors for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrandMode {
    /// The dark appearance.
    Dark,
    /// The light appearance.
    Light,
}

impl BrandMode {
    /// The key of this mode in a pack and the name of its shipped theme.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }
}

/// A glyph symbol with its resolved color.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrandGlyph<'a> {
    /// The symbol text.
    pub symbol: &'a str,
    /// Resolved color text.
    pub color: &'a str,
}

/// The selected mode's resolved values, borrowed from the pack.
#[derive(Debug, Clone)]
pub struct BrandPresentation<'a> {
    /// Pack label.
    pub name: &'a str,
    /// Displayed product name.
    pub wordmark: &'a str,
    /// Displayed tagline.
    pub tagline: &'a str,
    /// Resolved color text by role, in lexical role order.
    pub colors: BTreeMap<&'a str, &'a str>,
    /// Font roles.
    pub fonts: &'a BTreeMap<String, BrandFont>,
    /// Type roles.
    pub typography: &'a BTreeMap<String, BrandType>,
    /// Spacing in CSS pixels by role.
    pub spacing: &'a BTreeMap<String, f64>,
    /// Radii in CSS pixels by role.
    pub radii: &'a BTreeMap<String, f64>,
    /// Mark paths and measurements.
    pub mark: &'a BrandMark,
    /// Glyphs with resolved colors.
    pub glyphs: BTreeMap<&'a str, BrandGlyph<'a>>,
    /// Resolved color text by ANSI slot.
    pub ansi: BTreeMap<&'a str, &'a str>,
    /// Resolved color text by truecolor name.
    pub truecolor: BTreeMap<&'a str, &'a str>,
}

impl PackData {
    /// Resolve the selected mode's colors, fonts, glyphs and terminal aliases.
    ///
    /// # Errors
    /// Returns a missing-mode, palette, color, font or color-role error for the first failing reference.
    pub fn presentation(&self, mode: BrandMode) -> Result<BrandPresentation<'_>, BrandError> {
        let pack = self;
        let mode_data = pack
            .modes
            .get(mode.name())
            .ok_or(BrandError::MissingMode(mode))?;
        let colors = mode_data
            .colors
            .iter()
            .map(|(role, key)| Ok((role.as_str(), swatch(&pack.palette, key)?)))
            .collect::<Result<BTreeMap<_, _>, BrandError>>()?;
        if let Some(kind) = pack
            .typography
            .values()
            .find(|kind| !pack.fonts.contains_key(&kind.font))
        {
            return Err(BrandError::MissingFont(kind.font.clone()));
        }
        let glyphs = pack
            .glyphs
            .iter()
            .map(|(role, glyph)| {
                let color = role_color(&colors, &glyph.color)?;
                Ok((
                    role.as_str(),
                    BrandGlyph {
                        symbol: &glyph.symbol,
                        color,
                    },
                ))
            })
            .collect::<Result<_, BrandError>>()?;
        Ok(BrandPresentation {
            name: &pack.name,
            wordmark: &pack.wordmark,
            tagline: &pack.tagline,
            fonts: &pack.fonts,
            typography: &pack.typography,
            spacing: &pack.spacing,
            radii: &pack.radii,
            mark: &pack.mark,
            glyphs,
            ansi: aliases(&colors, &pack.terminal.ansi)?,
            truecolor: aliases(&colors, &pack.terminal.truecolor)?,
            colors,
        })
    }
}

/// Resolve a palette key to its six-digit hex color text.
fn swatch<'a>(palette: &'a BTreeMap<String, String>, key: &str) -> Result<&'a str, BrandError> {
    let color = palette
        .get(key)
        .ok_or_else(|| BrandError::MissingPaletteColor(key.to_owned()))?;
    let digits = color.strip_prefix('#').unwrap_or("");
    if digits.len() == 6 && digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(color)
    } else {
        Err(BrandError::InvalidColor(color.clone()))
    }
}

/// Resolve a role of the selected mode.
///
/// # Errors
/// Returns a missing-color-role error when `role` is absent.
pub fn role_color<'a>(colors: &BTreeMap<&str, &'a str>, role: &str) -> Result<&'a str, BrandError> {
    colors
        .get(role)
        .copied()
        .ok_or_else(|| BrandError::MissingColorRole(role.to_owned()))
}

/// Resolve each alias to the color of its role.
fn aliases<'a>(
    colors: &BTreeMap<&str, &'a str>,
    table: &'a BTreeMap<String, String>,
) -> Result<BTreeMap<&'a str, &'a str>, BrandError> {
    table
        .iter()
        .map(|(name, role)| Ok((name.as_str(), role_color(colors, role)?)))
        .collect()
}
