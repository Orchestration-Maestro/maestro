//! CSS custom properties for a resolved presentation.
use crate::pack::BrandFont;
use crate::presentation::BrandPresentation;
use maestro_text::css::{escape_control, string};
use std::collections::BTreeMap;

/// Generic family keywords, which stay unquoted.
const GENERIC_FAMILIES: [&str; 13] = [
    "serif",
    "sans-serif",
    "monospace",
    "cursive",
    "fantasy",
    "system-ui",
    "ui-serif",
    "ui-sans-serif",
    "ui-monospace",
    "ui-rounded",
    "math",
    "emoji",
    "fangsong",
];

impl BrandPresentation<'_> {
    /// Declaration names and values for the colors, font stacks, type, spacing and radii.
    ///
    /// Names and quoted text are CSS-escaped; `<` is a hexadecimal escape so a value can sit in
    /// a style element. The result holds no selectors, font downloads or layout rules.
    #[must_use]
    pub fn css_properties(&self) -> BTreeMap<String, String> {
        let mut out = BTreeMap::new();
        let mut put = |kind: &str, role: &str, suffix: &str, value: String| {
            out.insert(
                format!("--maestro-{kind}-{}{suffix}", identifier(role)),
                value,
            );
        };
        for (role, color) in &self.colors {
            put("color", role, "", (*color).to_owned());
        }
        for (role, font) in self.fonts {
            put("font", role, "", font_stack(font));
        }
        for (role, kind) in self.typography {
            let stack = self
                .fonts
                .get(&kind.font)
                .map(font_stack)
                .unwrap_or_default();
            put("type", role, "-font", stack);
            put("type", role, "-size", format!("{}px", kind.size));
            put("type", role, "-line-height", kind.line_height.to_string());
            put("type", role, "-weight", kind.weight.to_string());
        }
        for (role, value) in self.spacing {
            put("spacing", role, "", format!("{value}px"));
        }
        for (role, value) in self.radii {
            put("radii", role, "", format!("{value}px"));
        }
        out
    }
}

/// The family and its fallbacks, named families quoted and generic keywords bare.
#[must_use]
pub fn font_stack(font: &BrandFont) -> String {
    std::iter::once(&font.family)
        .chain(&font.fallbacks)
        .map(|name| {
            if GENERIC_FAMILIES
                .iter()
                .any(|keyword| keyword.eq_ignore_ascii_case(name))
            {
                name.clone()
            } else {
                string(name)
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Serialize the role part of a custom property name.
fn identifier(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if escape_control(&mut out, c) {
            continue;
        }
        if !(c.is_ascii_alphanumeric() || matches!(c, '-' | '_') || !c.is_ascii()) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}
