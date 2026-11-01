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
    if env(maestro_tui::TERM_PROGRAM).as_deref() == Some("Apple_Terminal")
        || term == "screen"
        || term.starts_with("screen-")
        || term.starts_with("screen.")
    {
        return ColorMode::Color256;
    }
    ColorMode::Truecolor
}
/// Whether text written for the terminal takes colour, decided on standard output, the
/// process environment and the platform.
#[must_use]
pub fn supports_color() -> bool {
    color_supported(
        io::stdout().is_terminal(),
        |name| std::env::var_os(name).map(|value| value.to_string_lossy().into_owned()),
        std::env::consts::OS,
    )
}
/// Decide colour in order: a `FORCE_COLOR` of `0` or `false` forbids it, an Azure Pipelines
/// agent allows it, a captured stream without `FORCE_COLOR` forbids it, and then the terminal,
/// platform, continuous-integration service and terminal names decide, falling back to
/// whether `FORCE_COLOR` asked for colour.
fn color_supported(
    stdout_is_terminal: bool,
    env: impl Fn(&str) -> Option<String>,
    os: &str,
) -> bool {
    let force = env("FORCE_COLOR").map(|level| match level.as_str() {
        "true" | "" => 1,
        "false" => 0,
        // Text that is not a whole number still counts as set, without asking for colour.
        level => level.parse::<i64>().unwrap_or(-1),
    });
    if force == Some(0) {
        return false;
    }
    if env("TF_BUILD").is_some() && env("AGENT_NAME").is_some() {
        return true;
    }
    if !stdout_is_terminal && force.is_none() {
        return false;
    }
    let forced = force.is_some_and(|level| level > 0);
    let term = env("TERM").unwrap_or_default();
    if term == "dumb" {
        return forced;
    }
    if os == "windows" {
        return true;
    }
    if env("CI").is_some() {
        return CI_SERVICES.iter().any(|name| env(name).is_some())
            || env("CI_NAME").as_deref() == Some("codeship")
            || forced;
    }
    if let Some(version) = env("TEAMCITY_VERSION") {
        return teamcity_colors(&version);
    }
    env("COLORTERM").is_some()
        || matches!(
            env(maestro_tui::TERM_PROGRAM).as_deref(),
            Some("iTerm.app" | "Apple_Terminal")
        )
        || term == "wezterm"
        || term_names_color(&term)
        || forced
}
/// Continuous-integration services whose logs show colour.
const CI_SERVICES: [&str; 8] = [
    "GITHUB_ACTIONS",
    "GITEA_ACTIONS",
    "CIRCLECI",
    "TRAVIS",
    "APPVEYOR",
    "GITLAB_CI",
    "BUILDKITE",
    "DRONE",
];
/// The `TeamCity` build log shows colour from version 9.1.
fn teamcity_colors(version: &str) -> bool {
    let digits = |text: &str| !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit());
    let Some((major, rest)) = version.split_once('.') else {
        return false;
    };
    if major.len() >= 2 && digits(major) {
        return true;
    }
    major == "9"
        && rest
            .split_once('.')
            .is_some_and(|(minor, _)| digits(minor) && minor.bytes().any(|byte| byte != b'0'))
}
/// Whether a `TERM` name, read case-insensitively, belongs to a colour terminal family
/// (`xterm-kitty` and `xterm-ghostty` among the `xterm` names).
fn term_names_color(term: &str) -> bool {
    let term = term.to_ascii_lowercase();
    term.ends_with("-256")
        || term.ends_with("-256color")
        || ["screen", "xterm", "vt100", "vt220", "rxvt"]
            .iter()
            .any(|family| term.starts_with(family))
        || ["color", "ansi", "cygwin", "linux"]
            .iter()
            .any(|part| term.contains(part))
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
