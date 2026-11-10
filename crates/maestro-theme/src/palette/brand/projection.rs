//! Terminal theme document mapping from a shared brand presentation.
use super::{BrandMode, BrandPack, BrandPresentation};
use crate::palette::ThemeError;
use maestro_brand::role_color;
use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// The ordered terminal-key to role table and the export aliases.
const ROLES: &str = include_str!("../../../assets/theme/brand-roles.json");
/// The schema the shipped theme documents declare.
const SCHEMA: &str = "https://raw.githubusercontent.com/Orchestration-Maestro/maestro/main/crates/maestro-theme/assets/theme/theme-schema.json";

/// Terminal keys with their roles, and the export fields with theirs.
#[derive(serde::Deserialize)]
struct Roles {
    /// Terminal color key and role, in the established key order; an empty role is the terminal default.
    colors: Vec<(String, String)>,
    /// Export field and role.
    export: Vec<(String, String)>,
}

impl BrandPack {
    /// Resolve the selected presentation through the shared brand owner.
    ///
    /// # Errors
    /// Returns the failures of [`maestro_brand::PackData::presentation`].
    pub fn presentation(&self, mode: BrandMode) -> Result<BrandPresentation<'_>, ThemeError> {
        self.data.presentation(mode).map_err(ThemeError::from)
    }

    /// The shipped-format theme document for `mode`: two-space JSON ending in a newline.
    ///
    /// # Errors
    /// Returns the failures of [`BrandPack::presentation`], or `Missing brand color role` for a
    /// terminal or export key whose role the mode does not define.
    pub fn theme_json(&self, mode: BrandMode) -> Result<String, ThemeError> {
        let presentation = self.presentation(mode)?;
        let roles = roles()?;
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

/// Require a role of the selected mode, or the empty terminal-default role.
fn declared(colors: &BTreeMap<&str, &str>, role: &str) -> Result<(), ThemeError> {
    if role.is_empty() {
        return Ok(());
    }
    role_color(colors, role).map(drop).map_err(ThemeError::from)
}

/// Parse the single shipped role table for all projections.
fn roles() -> Result<Roles, ThemeError> {
    serde_json::from_str(ROLES).map_err(|cause| ThemeError::caused_by(cause.to_string(), cause))
}

/// Project emitted syntax classes through the existing brand-role aliases.
pub(crate) fn syntax_css() -> Result<String, ThemeError> {
    use std::fmt::Write as _;
    let mut css = String::new();
    for (key, role) in roles()?.colors {
        if let Some(class) = crate::palette::code::syntax_class(&key) {
            let _ = write!(css, ".hljs-{class}{{color:var(--maestro-color-{role});}}");
        }
    }
    Ok(css)
}
