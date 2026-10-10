//! Custom-file theme admission and alias resolution.
use super::{ColorMode, ColorValue, Theme, ThemeBg, ThemeColor, ThemeError, ThemeOptions};
use num_traits::ToPrimitive;
use serde_json::Value;
use std::collections::HashSet;

/// Replaceable file and environment effects for theme loading.
pub trait ThemeOperations {
    /// Read the supplied authored path.
    ///
    /// # Errors
    /// Returns the adapter's I/O failure.
    fn read_to_string(&self, path: &str) -> std::io::Result<String>;
    /// Read one optional environment variable.
    fn environment(&self, name: &str) -> Option<String>;
}
/// Native file bytes decoded lossily as UTF-8, with native environment access.
#[cfg(not(target_arch = "wasm32"))]
pub struct NativeThemeOperations;
#[cfg(not(target_arch = "wasm32"))]
impl ThemeOperations for NativeThemeOperations {
    fn read_to_string(&self, path: &str) -> std::io::Result<String> {
        std::fs::read(path).map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
    }
    fn environment(&self, name: &str) -> Option<String> {
        std::env::var(name).ok()
    }
}
/// Read, validate, resolve and construct an independent terminal theme.
///
/// Explicit mode avoids environment lookup. The supplied path is retained verbatim.
///
/// ```
/// use maestro_theme::{ColorMode, NativeThemeOperations, ThemeColor, load_theme_from_path};
///
/// let path = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/theme/dark.json");
/// let theme = load_theme_from_path(path, Some(ColorMode::Truecolor), &NativeThemeOperations)?;
/// assert_eq!(theme.fg(&ThemeColor::Accent, "hi")?, "\x1b[38;2;138;190;183mhi\x1b[39m");
/// # Ok::<(), maestro_theme::ThemeError>(())
/// ```
///
/// # Errors
/// Returns native I/O failures, labeled JSON/schema errors, or color/alias errors.
pub fn load_theme_from_path(
    path: &str,
    mode: Option<ColorMode>,
    operations: &dyn ThemeOperations,
) -> Result<Theme, ThemeError> {
    let content = operations
        .read_to_string(path)
        .map_err(|cause| ThemeError {
            message: cause.to_string(),
            cause: Some(Box::new(cause)),
        })?;
    let json: Value = serde_json::from_str(&content).map_err(|cause| ThemeError {
        message: format!("Failed to parse theme {path}: {cause}"),
        cause: Some(Box::new(cause)),
    })?;
    validate(path, &json)?;
    let mode = mode.unwrap_or_else(|| detect_mode(operations));
    let mut fg = Vec::new();
    let mut bg = Vec::new();
    if let Some(colors) = json["colors"].as_object() {
        for (key, value) in colors {
            let color = resolve(value, &json["vars"])?;
            if is_background(key) {
                bg.push((ThemeBg::Named(key.clone()), color));
            } else {
                fg.push((ThemeColor::Named(key.clone()), color));
            }
        }
    }
    Theme::new(
        fg,
        bg,
        mode,
        ThemeOptions {
            name: json["name"].as_str().map(str::to_owned),
            source_path: Some(path.to_owned()),
        },
    )
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
fn detect_mode(operations: &dyn ThemeOperations) -> ColorMode {
    if matches!(
        operations.environment("COLORTERM").as_deref(),
        Some("truecolor" | "24bit")
    ) {
        return ColorMode::Truecolor;
    }
    if operations
        .environment("WT_SESSION")
        .is_some_and(|value| !value.is_empty())
    {
        return ColorMode::Truecolor;
    }
    let term = operations.environment("TERM").unwrap_or_default();
    if matches!(term.as_str(), "" | "dumb" | "linux") {
        return ColorMode::Color256;
    }
    if operations.environment("TERM_PROGRAM").as_deref() == Some("Apple_Terminal")
        || term == "screen"
        || term.starts_with("screen-")
        || term.starts_with("screen.")
    {
        return ColorMode::Color256;
    }
    ColorMode::Truecolor
}
/// Admit selected runtime fields with the native schema validator.
fn validate(path: &str, json: &Value) -> Result<(), ThemeError> {
    let schema: Value = serde_json::from_str(include_str!(
        "../../assets/theme/runtime-schema.json"
    ))
    .map_err(|cause| ThemeError {
        message: cause.to_string(),
        cause: Some(Box::new(cause)),
    })?;
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
fn resolve(value: &Value, vars: &Value) -> Result<ColorValue, ThemeError> {
    let mut value = authored(value)?;
    let mut visited = HashSet::new();
    loop {
        match value {
            ColorValue::String(ref name) if !name.is_empty() && !name.starts_with('#') => {
                if !visited.insert(name.clone()) {
                    return Err(ThemeError::message(format!(
                        "Circular variable reference detected: {name}"
                    )));
                }
                let target = vars.get(name).ok_or_else(|| {
                    ThemeError::message(format!("Variable reference not found: {name}"))
                })?;
                value = authored(target)?;
            }
            value => return Ok(value),
        }
    }
}
