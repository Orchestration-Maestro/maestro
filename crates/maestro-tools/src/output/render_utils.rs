//! Display strings shared by tool renderers.

use crate::utils::shell::sanitize_binary_output;
#[cfg(not(target_arch = "wasm32"))]
use crate::{
    output::render_parts::{paint, themed},
    utils::diagnostics::diagnostic,
};
use maestro_agent::AgentToolResult;
use maestro_models::{DiagnosticErrorInfo, UserBlock};
use maestro_theme::{Theme, ThemeColor, ThemeError};
use maestro_tui::{TerminalImage, get_image_dimensions, image_fallback};
#[cfg(not(target_arch = "wasm32"))]
use regex::Regex;
use serde_json::Value;

/// A tool result as renderers see it.
pub struct ToolRenderResultLike<'a, D> {
    /// Ordered content borrowed from the execution record.
    pub content: &'a [UserBlock],
    /// Borrowed details, preserving absence.
    pub details: Option<&'a D>,
    /// Termination hint supplied by the executed result, when present.
    pub terminate: Option<bool>,
    /// Error observation supplied by a rendering host.
    pub is_error: Option<bool>,
}
impl<D> ToolRenderResultLike<'_, D> {
    /// Borrow the same content and flags with a selected details representation.
    #[must_use]
    pub fn with_details<'a, T>(&'a self, details: Option<&'a T>) -> ToolRenderResultLike<'a, T> {
        ToolRenderResultLike {
            content: self.content,
            details,
            terminate: self.terminate,
            is_error: self.is_error,
        }
    }
}
impl<'a, D> From<&'a AgentToolResult<D>> for ToolRenderResultLike<'a, D> {
    fn from(result: &'a AgentToolResult<D>) -> Self {
        Self {
            content: &result.content,
            details: result.details.as_ref(),
            terminate: result.terminate,
            is_error: None,
        }
    }
}

/// Rows of a compact preview.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) const PREVIEW_LINES: usize = 10;

/// Borrow builtin arguments, treating a missing object as missing fields.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn render_args(args: Option<&Value>) -> &Value {
    args.unwrap_or(&Value::Null)
}

/// The path argument: `file_path` unless it is absent or null, then `path`.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn selected_path(args: &Value) -> Option<&Value> {
    args.get("file_path")
        .filter(|value| !value.is_null())
        .or_else(|| args.get("path"))
}

/// `lines` without the empty rows at the end; rows holding only whitespace stay.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn without_trailing_empty<T: AsRef<str>>(lines: &[T]) -> &[T] {
    let kept = lines
        .iter()
        .rposition(|line| !line.as_ref().is_empty())
        .map_or(0, |last| last + 1);
    &lines[..kept]
}

/// The path in a call title: accent for a path, `...` for an empty, absent or null one,
/// `[invalid arg]` for any other value that is not a string.
///
/// # Errors
/// Returns a home lookup or theme failure.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn path_display(
    path: Option<&Value>,
    theme: &Theme,
) -> Result<String, DiagnosticErrorInfo> {
    let path = str(path)
        .map(|raw| shorten_path(Some(raw)))
        .transpose()
        .map_err(|error| diagnostic(&error))?;
    let invalid = themed(invalid_arg_text(theme))?;
    match path.as_deref() {
        None => Ok(invalid),
        Some("") => paint(theme, &ThemeColor::ToolOutput, "..."),
        Some(path) => paint(theme, &ThemeColor::Accent, path),
    }
}

/// The string at `value`, `Some("")` when it is absent or null, `None` for any other type.
#[must_use]
pub fn str(value: Option<&Value>) -> Option<&str> {
    match value {
        None | Some(Value::Null) => Some(""),
        Some(value) => value.as_str(),
    }
}

/// Show the home directory as `~`, for `path` equal to it or below it; absent text is empty.
///
/// A home directory that is empty or the root directory abbreviates nothing, so `path` is
/// shown as given.
///
/// # Errors
/// Returns the home lookup failure when `path` is present.
#[cfg(not(target_arch = "wasm32"))]
pub fn shorten_path(path: Option<&str>) -> std::io::Result<String> {
    let Some(path) = path else {
        return Ok(String::new());
    };
    let home = crate::path_utils::expand_path("~")?;
    if home.is_empty() || home == "/" {
        return Ok(path.to_owned());
    }
    let below = path.strip_prefix(&home).filter(|rest| {
        rest.is_empty()
            || rest.starts_with(std::path::is_separator)
            || home.ends_with(std::path::is_separator)
    });
    Ok(below.map_or_else(|| path.to_owned(), |rest| format!("~{rest}")))
}

/// Replace every TAB with three spaces.
#[must_use]
pub fn replace_tabs(text: &str) -> String {
    text.replace('\t', "   ")
}

/// Remove every carriage return.
#[must_use]
pub fn normalize_display_text(text: &str) -> String {
    text.replace('\r', "")
}

/// Remove every match of the escape expression.
///
/// # Errors
/// Returns the failure to compile the escape expression.
pub fn strip_ansi(text: &str) -> Result<String, DiagnosticErrorInfo> {
    maestro_text::terminal::strip_ansi(text)
        .map_err(|error| DiagnosticErrorInfo::error(error.to_string()))
}

/// The text blocks of `result` cleaned for display and joined by LF, then, when the
/// terminal cannot show images or `show_images` is false, one fallback line per image.
///
/// # Errors
/// Returns the failure to compile the escape expression.
pub fn get_text_output<D>(
    result: Option<&ToolRenderResultLike<'_, D>>,
    show_images: bool,
    terminal_image: &TerminalImage,
) -> Result<String, DiagnosticErrorInfo> {
    let Some(result) = result else {
        return Ok(String::new());
    };
    let texts = result.content.iter().filter_map(|block| match block {
        UserBlock::Text(text) => Some(text.text.as_str()),
        UserBlock::Image(_) => None,
    });
    let cleaned = texts
        .map(|text| {
            strip_ansi(text).map(|plain| normalize_display_text(&sanitize_binary_output(&plain)))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let output = cleaned.join("\n");
    let images_shown = terminal_image.get_capabilities().images.is_some() && show_images;
    let indicators: Vec<String> = result
        .content
        .iter()
        .filter_map(|block| match block {
            UserBlock::Image(image) => Some(image),
            UserBlock::Text(_) => None,
        })
        .filter(|_| !images_shown)
        .map(|image| {
            let dimensions = (!image.data.is_empty() && !image.mime_type.is_empty())
                .then(|| get_image_dimensions(&image.data, &image.mime_type))
                .flatten();
            image_fallback(&image.mime_type, dimensions, None)
        })
        .collect();
    if indicators.is_empty() {
        return Ok(output);
    }
    let indicators = indicators.join("\n");
    Ok(if output.is_empty() {
        indicators
    } else {
        format!("{output}\n{indicators}")
    })
}

/// `[invalid arg]` in the error style.
///
/// # Errors
/// Returns the theme's failure when it lacks the error color.
pub fn invalid_arg_text(theme: &Theme) -> Result<String, ThemeError> {
    theme.fg(&ThemeColor::Error, "[invalid arg]")
}

/// Borrow the single compiled escape grammar.
#[cfg(not(target_arch = "wasm32"))]
fn ansi() -> Result<&'static Regex, DiagnosticErrorInfo> {
    maestro_text::terminal::ansi_regex()
        .map_err(|error| DiagnosticErrorInfo::error(error.to_string()))
}

/// Retains only an unfinished escape between decoded chunks.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
pub(crate) struct AnsiStream {
    /// Unemitted suffix that may complete an escape.
    pending: String,
}
#[cfg(not(target_arch = "wasm32"))]
impl AnsiStream {
    /// Strip completed sequences, holding a suffix that accepts an escape completion.
    pub(crate) fn push(&mut self, text: &str) -> Result<String, DiagnosticErrorInfo> {
        self.pending.push_str(text);
        let end = self.pending.len();
        // shortcut: rescans the held unfinished escape on each chunk, quadratic only for one escape split across many tiny chunks; upgrade to an incremental recognizer if a real producer shows it
        self.pending.push_str("m\u{7}");
        let split = ansi()?
            .find_iter(&self.pending)
            .find(|found| found.start() < end && found.end() > end)
            .map_or(end, |found| found.start());
        self.pending.truncate(end);
        let rest = self.pending.split_off(split);
        let emitted = std::mem::replace(&mut self.pending, rest);
        strip_ansi(&emitted)
    }
    /// Apply whole-text stripping to an unfinished suffix at settlement.
    pub(crate) fn finish(&mut self) -> Result<String, DiagnosticErrorInfo> {
        strip_ansi(&std::mem::take(&mut self.pending))
    }
}

/// Format a line number or count the way the model has always seen it.
#[must_use]
pub fn num(value: f64) -> String {
    ryu_js::Buffer::new().format(value).to_owned()
}

/// `:start-end` for the offset and limit arguments, empty when neither is present; a
/// non-numeric offset counts as 1 and a non-numeric limit as 0, and an end of 0 is omitted.
#[must_use]
pub fn format_read_line_range(args: &Value) -> String {
    let (offset, limit) = (args.get("offset"), args.get("limit"));
    if offset.is_none() && limit.is_none() {
        return String::new();
    }
    let start = offset.and_then(Value::as_f64).unwrap_or(1.0);
    let end = limit
        .map(|limit| (start + limit.as_f64().unwrap_or(0.0)) - 1.0)
        .filter(|end| *end != 0.0 && !end.is_nan());
    let tail = end.map_or_else(String::new, |end| format!("-{}", num(end)));
    format!(":{}{tail}", num(start))
}

/// Borrow only an admitted numeric renderer field.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn number(details: Option<&Value>, key: &str) -> Option<f64> {
    details?.get(key)?.as_f64()
}
/// Select a boolean before applying the shared JSON truth rule.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn flag(details: Option<&Value>, key: &str) -> bool {
    details
        .and_then(|details| details.get(key))
        .filter(|value| value.is_boolean())
        .is_some_and(maestro_json::is_truthy)
}
/// Numeric text for a renderer field, preserving absent-field spelling.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn number_text(details: Option<&Value>, key: &str) -> String {
    number(details, key).map_or_else(|| "undefined".to_owned(), num)
}
/// Sparse borrowed truncation metadata; execution records remain independent.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Copy)]
pub(crate) struct TruncationView<'a>(pub(crate) &'a Value);
#[cfg(not(target_arch = "wasm32"))]
impl<'a> TruncationView<'a> {
    /// Select only active truncation, ignoring unread fields.
    pub(crate) fn selected(details: Option<&'a Value>) -> Option<Self> {
        let cut = details?.get("truncation")?;
        flag(Some(cut), "truncated").then_some(Self(cut))
    }
    /// Existing default byte budget, without narrowing supplied numeric values.
    pub(crate) fn max_bytes(self) -> f64 {
        number(Some(self.0), "maxBytes")
            .unwrap_or(crate::output::render_parts::float(crate::DEFAULT_MAX_BYTES))
    }
    /// Whether the line limit selected the output.
    pub(crate) fn by_lines(self) -> bool {
        self.0.get("truncatedBy").and_then(Value::as_str) == Some("lines")
    }
}
