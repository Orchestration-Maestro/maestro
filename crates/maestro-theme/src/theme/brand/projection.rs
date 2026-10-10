//! Resolution of the selected mode's roles into colors, glyphs and theme documents.
use super::data::{BrandFont, BrandMark, BrandType};
use super::{BrandMode, BrandPack};
use crate::theme::ThemeError;
use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// The ordered terminal-key to role table and the export aliases.
const ROLES: &str = include_str!("../../../assets/theme/brand-roles.json");
/// The schema the shipped theme documents declare.
const SCHEMA: &str = "https://raw.githubusercontent.com/Orchestration-Maestro/maestro/main/crates/maestro-theme/assets/theme/theme-schema.json";

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

/// Terminal keys with their roles, and the export fields with theirs.
#[derive(serde::Deserialize)]
struct Roles {
    /// Terminal color key and role, in the established key order; an empty role is the terminal default.
    colors: Vec<(String, String)>,
    /// Export field and role.
    export: Vec<(String, String)>,
}

impl BrandPack {
    /// Resolve the selected mode's colors, fonts, glyphs and terminal aliases.
    ///
    /// # Errors
    /// Returns `Missing brand mode`, `Missing brand palette color`, `Invalid brand color`,
    /// `Missing brand font` or `Missing brand color role` for the first failing reference.
    pub fn presentation(&self, mode: BrandMode) -> Result<BrandPresentation<'_>, ThemeError> {
        let pack = &self.data;
        let mode_data = pack
            .modes
            .get(mode.name())
            .ok_or_else(|| ThemeError::message(format!("Missing brand mode: {}", mode.name())))?;
        let colors = mode_data
            .colors
            .iter()
            .map(|(role, key)| Ok((role.as_str(), swatch(&pack.palette, key)?)))
            .collect::<Result<BTreeMap<_, _>, ThemeError>>()?;
        if let Some(kind) = pack
            .typography
            .values()
            .find(|kind| !pack.fonts.contains_key(&kind.font))
        {
            return Err(ThemeError::message(format!(
                "Missing brand font: {}",
                kind.font
            )));
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
            .collect::<Result<_, ThemeError>>()?;
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

    /// The shipped-format theme document for `mode`: two-space JSON ending in a newline.
    ///
    /// # Errors
    /// Returns the failures of [`BrandPack::presentation`], or `Missing brand color role` for a
    /// terminal or export key whose role the mode does not define.
    pub fn theme_json(&self, mode: BrandMode) -> Result<String, ThemeError> {
        let presentation = self.presentation(mode)?;
        let roles: Roles = serde_json::from_str(ROLES)
            .map_err(|cause| ThemeError::caused_by(cause.to_string(), cause))?;
        let aliases = |table: Vec<(String, String)>| -> Result<Map<String, Value>, ThemeError> {
            table
                .into_iter()
                .map(|(key, role)| {
                    declared(&presentation.colors, &role)?;
                    Ok((key, Value::String(role)))
                })
                .collect()
        };
        let vars = presentation
            .colors
            .iter()
            .map(|(role, color)| ((*role).to_owned(), Value::from(*color)))
            .collect();
        let mut document = Map::new();
        document.insert("$schema".to_owned(), SCHEMA.into());
        document.insert("name".to_owned(), mode.name().into());
        document.insert("vars".to_owned(), Value::Object(vars));
        document.insert("colors".to_owned(), Value::Object(aliases(roles.colors)?));
        document.insert("export".to_owned(), Value::Object(aliases(roles.export)?));
        let text = serde_json::to_string_pretty(&Value::Object(document))
            .map_err(|cause| ThemeError::caused_by(cause.to_string(), cause))?;
        Ok(text + "\n")
    }
}

/// Resolve a palette key to its six-digit hex color text.
fn swatch<'a>(palette: &'a BTreeMap<String, String>, key: &str) -> Result<&'a str, ThemeError> {
    let color = palette
        .get(key)
        .ok_or_else(|| ThemeError::message(format!("Missing brand palette color: {key}")))?;
    let digits = color.strip_prefix('#').unwrap_or("");
    if digits.len() == 6 && digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(color)
    } else {
        Err(ThemeError::message(format!("Invalid brand color: {color}")))
    }
}

/// Require a role of the selected mode, or the empty terminal-default role.
fn declared(colors: &BTreeMap<&str, &str>, role: &str) -> Result<(), ThemeError> {
    if role.is_empty() {
        return Ok(());
    }
    role_color(colors, role).map(drop)
}

/// Resolve a role of the selected mode.
pub(super) fn role_color<'a>(
    colors: &BTreeMap<&str, &'a str>,
    role: &str,
) -> Result<&'a str, ThemeError> {
    colors
        .get(role)
        .copied()
        .ok_or_else(|| ThemeError::message(format!("Missing brand color role: {role}")))
}

/// Resolve each alias to the color of its role.
fn aliases<'a>(
    colors: &BTreeMap<&str, &'a str>,
    table: &'a BTreeMap<String, String>,
) -> Result<BTreeMap<&'a str, &'a str>, ThemeError> {
    table
        .iter()
        .map(|(name, role)| Ok((name.as_str(), role_color(colors, role)?)))
        .collect()
}
