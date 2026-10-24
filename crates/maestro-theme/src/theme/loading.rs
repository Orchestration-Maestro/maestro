//! Custom-file theme admission and alias resolution.
use super::{ColorMode, ColorValue, Theme, ThemeBg, ThemeColor, ThemeError, ThemeOptions};
use maestro_json::canonical_entries;
use num_traits::ToPrimitive;
use serde_json::Value;
use std::collections::HashSet;
use std::io::{self, IsTerminal};

/// Read a file as UTF-8 text, replacing invalid sequences.
///
/// # Errors
/// Returns the native I/O failure.
pub(super) fn read_text(path: &str) -> std::io::Result<String> {
    std::fs::read(path).map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
}
/// Read, validate, resolve and construct an independent terminal theme.
///
/// Explicit mode avoids environment lookup. The supplied path is retained verbatim.
///
/// ```
/// use maestro_theme::{ColorMode, ThemeColor, load_theme_from_path};
///
/// let path = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/theme/dark.json");
/// let theme = load_theme_from_path(path, Some(ColorMode::Truecolor))?;
/// assert_eq!(theme.fg(&ThemeColor::Accent, "hi")?, "\x1b[38;2;217;160;102mhi\x1b[39m");
/// # Ok::<(), maestro_theme::ThemeError>(())
/// ```
///
/// # Errors
/// Returns native I/O failures, labeled JSON/schema errors, or color/alias errors.
pub fn load_theme_from_path(path: &str, mode: Option<ColorMode>) -> Result<Theme, ThemeError> {
    let content = read_text(path).map_err(ThemeError::io)?;
    let json = parse_custom(path, &content)?;
    build_theme(
        &json,
        mode,
        ThemeOptions {
            source_path: Some(path.to_owned()),
            ..ThemeOptions::default()
        },
    )
}
/// Parse theme text and admit it against the runtime schema.
pub(super) fn parse_custom(label: &str, content: &str) -> Result<Value, ThemeError> {
    let json = serde_json::from_str(content).map_err(|cause| {
        ThemeError::caused_by(format!("Failed to parse theme {label}: {cause}"), cause)
    })?;
    validate(label, &json)?;
    Ok(json)
}
/// Resolve and construct a theme from a parsed document, borrowing its colors.
///
/// The document's name is authored; `options` supplies the rest of the metadata.
pub(super) fn build_theme(
    json: &Value,
    mode: Option<ColorMode>,
    mut options: ThemeOptions,
) -> Result<Theme, ThemeError> {
    let mode = mode.unwrap_or_else(detect_mode);
    let mut fg = Vec::new();
    let mut bg = Vec::new();
    let colors = json["colors"]
        .as_object()
        .ok_or_else(|| ThemeError::message("Theme colors must be an object".to_owned()))?;
    for (key, value) in canonical_entries(colors) {
        let color = resolve(value, &json["vars"])?;
        if is_background(key) {
            bg.push((ThemeBg::Named(key.clone()), color));
        } else {
            fg.push((ThemeColor::Named(key.clone()), color));
        }
    }
    options.name = json["name"].as_str().map(str::to_owned);
    Theme::new(fg, bg, mode, options)
}
/// The six authored background keys.
fn is_background(key: &str) -> bool {
    matches!(
        key,
        "selectedBg"
            | "userMessageBg"
            | "customMessageBg"
            | "toolPendingBg"
            | "toolSuccessBg"
            | "toolErrorBg"
    )
}
/// Sample environment in terminal capability precedence order.
fn detect_mode() -> ColorMode {
    let env = |name| std::env::var(name).ok();
    if matches!(env("COLORTERM").as_deref(), Some("truecolor" | "24bit")) {
        return ColorMode::Truecolor;
    }
    if env("WT_SESSION").is_some_and(|value| !value.is_empty()) {
        return ColorMode::Truecolor;
    }
    let term = env("TERM").unwrap_or_default();
    if matches!(term.as_str(), "" | "dumb" | "linux") {
        return ColorMode::Color256;
    }
    if env("TERM_PROGRAM").as_deref() == Some("Apple_Terminal")
        || term == "screen"
        || term.starts_with("screen-")
        || term.starts_with("screen.")
    {
        return ColorMode::Color256;
    }
    ColorMode::Truecolor
}
/// Whether text written for the terminal takes colour, decided on standard output and the
/// process environment.
#[must_use]
pub fn supports_color() -> bool {
    color_supported(io::stdout().is_terminal(), |name| std::env::var(name).ok())
}
/// `FORCE_COLOR` decides first: `true`, empty or a positive level forces colour, `false` or
/// `0` forbids it. Otherwise a terminal takes colour unless `TERM` is `dumb`.
fn color_supported(stdout_is_terminal: bool, env: impl Fn(&str) -> Option<String>) -> bool {
    let forced = env("FORCE_COLOR").and_then(|level| match level.as_str() {
        "true" | "" => Some(true),
        "false" => Some(false),
        level => level.parse::<u32>().ok().map(|level| level > 0),
    });
    forced.unwrap_or_else(|| stdout_is_terminal && env("TERM").as_deref() != Some("dumb"))
}
/// Admit selected runtime fields with the native schema validator.
fn validate(path: &str, json: &Value) -> Result<(), ThemeError> {
    let schema: Value =
        serde_json::from_str(include_str!("../../assets/theme/runtime-schema.json"))
            .map_err(|cause| ThemeError::caused_by(cause.to_string(), cause))?;
    let validator = jsonschema::validator_for(&schema)
        .map_err(|cause| ThemeError::message(cause.to_string()))?;
    let mut missing = std::collections::BTreeSet::new();
    let mut others = Vec::new();
    for error in validator.iter_errors(json) {
        let location = error.instance_path().to_string();
        if let jsonschema::error::ValidationErrorKind::Required { property } = error.kind()
            && location == "/colors"
            && let Some(name) = property.as_str()
        {
            missing.insert(name.to_owned());
        } else {
            let location = if location.is_empty() { "/" } else { &location };
            others.push(format!("  - {location}: {error}"));
        }
    }
    if missing.is_empty() && others.is_empty() {
        return Ok(());
    }
    let mut message = format!("Invalid theme \"{path}\":\n");
    if !missing.is_empty() {
        message.push_str("\nMissing required color tokens:\n");
        message.push_str(
            &missing
                .into_iter()
                .map(|name| format!("  - {name}"))
                .collect::<Vec<_>>()
                .join("\n"),
        );
        message.push_str("\n\nPlease add these colors to your theme's \"colors\" object.");
        message.push_str("\nSee the built-in themes (dark.json, light.json) for reference values.");
    }
    if !others.is_empty() {
        message.push_str("\n\nOther errors:\n");
        message.push_str(&others.join("\n"));
    }
    Err(ThemeError::message(message))
}
/// Select a typed authored color after runtime admission.
fn authored(value: &Value) -> Result<ColorValue, ThemeError> {
    if let Some(string) = value.as_str() {
        return Ok(ColorValue::String(string.to_owned()));
    }
    if let Some(index) = value
        .as_f64()
        .filter(|number| number.fract() == 0.0)
        .and_then(|number| number.to_u8())
    {
        return Ok(ColorValue::Index(index));
    }
    Err(ThemeError::message(format!("Invalid color value: {value}")))
}
/// Follow an immutable alias chain, rejecting revisits within this color only.
pub(super) fn resolve(value: &Value, vars: &Value) -> Result<ColorValue, ThemeError> {
    let mut current = value;
    let mut visited = HashSet::new();
    while let Some(name) = current
        .as_str()
        .filter(|name| !name.is_empty() && !name.starts_with('#'))
    {
        if !visited.insert(name) {
            return Err(ThemeError::message(format!(
                "Circular variable reference detected: {name}"
            )));
        }
        current = vars
            .get(name)
            .ok_or_else(|| ThemeError::message(format!("Variable reference not found: {name}")))?;
    }
    authored(current)
}
#[cfg(test)]
mod tests;
