//! Compiled author terminal text observations.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::number;
use maestro_extensions_wasm::{ExtensionAPI, ExtensionResult};
use serde_json::{Value, json};
/// Selects the terminal observation family.
pub(super) fn observe(api: &ExtensionAPI, operation: &str, args: &Value) -> ExtensionResult<Value> {
    match operation {
        "u.visibleWidth"
        | "u.normalizeTerminalOutput"
        | "u.extractAnsiCode"
        | "u.isWhitespaceChar"
        | "u.isPunctuationChar"
        | "composition" => basic(api, operation, args),
        "u.wrapTextWithAnsi" | "u.applyBackgroundToLine" | "u.truncateToWidth" => {
            width(api, operation, args)
        }
        _ => columns(api, operation, args),
    }
}

/// Reads the basic observation through the public author facade.
fn basic(api: &ExtensionAPI, operation: &str, args: &Value) -> ExtensionResult<Value> {
    let text = args[0].as_str().ok_or("missing text")?;
    match Some(operation) {
        Some("u.visibleWidth") => Ok(json!(maestro_extensions_wasm::utils::visible_width(
            api, text
        )?)),
        Some("u.normalizeTerminalOutput") => Ok(json!(
            maestro_extensions_wasm::utils::normalize_terminal_output(api, text)?
        )),
        Some("u.extractAnsiCode") => {
            let pos = usize::try_from(args[1].as_u64().ok_or("missing position")?)
                .map_err(|e| e.to_string())?;
            Ok(json!(maestro_extensions_wasm::utils::extract_ansi_code(
                api, text, pos
            )?))
        }
        Some("u.isWhitespaceChar") => Ok(json!(
            maestro_extensions_wasm::utils::is_whitespace_char(api, text)?
        )),
        Some("u.isPunctuationChar") => Ok(json!(
            maestro_extensions_wasm::utils::is_punctuation_char(api, text)?
        )),
        Some("composition") => Ok(json!({
            "width": maestro_extensions_wasm::utils::visible_width(api, text)?,
            "normalized": maestro_extensions_wasm::utils::normalize_terminal_output(api, "ำຳ")?,
            "ansi": maestro_extensions_wasm::utils::extract_ansi_code(api, "é\u{1b}[0m", 2)?,
            "whitespace": maestro_extensions_wasm::utils::is_whitespace_char(api, "\u{feff}")?,
            "punctuation": maestro_extensions_wasm::utils::is_punctuation_char(api, "a?")?,
        })),
        _ => Err("unknown helper observation".into()),
    }
}

/// Reads the width observation through the public author facade.
fn width(api: &ExtensionAPI, operation: &str, args: &Value) -> ExtensionResult<Value> {
    let text = args[0].as_str().ok_or("missing text")?;
    match Some(operation) {
        Some("u.wrapTextWithAnsi") => Ok(json!(
            maestro_extensions_wasm::utils::wrap_text_with_ansi(api, text, number(&args[1])?)?
        )),
        Some("u.applyBackgroundToLine") => {
            let mut calls = Vec::new();
            let value = maestro_extensions_wasm::utils::apply_background_to_line(
                api,
                text,
                number(&args[1])?,
                |padded| {
                    calls.push(padded.to_owned());
                    format!("\u{1b}[44m{padded}\u{1b}[0m")
                },
            )?;
            Ok(json!({"value":value,"calls":calls}))
        }
        Some("u.truncateToWidth") => {
            let mut options = maestro_extensions_wasm::utils::TruncateOptions::default();
            if let Some(ellipsis) = args[2].as_str() {
                options.ellipsis = ellipsis;
            }
            if let Some(pad) = args[3].as_bool() {
                options.pad = pad;
            }
            let value = maestro_extensions_wasm::utils::truncate_to_width(
                api,
                text,
                number(&args[1])?,
                options,
            )?;
            Ok(
                json!({"width":maestro_extensions_wasm::utils::visible_width(api, &value)?,"text":value}),
            )
        }
        _ => Err("unknown helper observation".into()),
    }
}

/// Reads the columns observation through the public author facade.
fn columns(api: &ExtensionAPI, operation: &str, args: &Value) -> ExtensionResult<Value> {
    let text = args[0].as_str().ok_or("missing text")?;
    match Some(operation) {
        Some(operation @ ("u.sliceWithWidth" | "u.sliceByColumn")) => {
            let (start, length, strict) = (
                number(&args[1])?,
                number(&args[2])?,
                args[3].as_bool().ok_or("missing strictness")?,
            );
            let slice =
                maestro_extensions_wasm::utils::slice_with_width(api, text, start, length, strict)?;
            let plain =
                maestro_extensions_wasm::utils::slice_by_column(api, text, start, length, strict)?;
            if plain != slice.text {
                return Err("column helpers disagree".into());
            }
            Ok(if operation == "u.sliceWithWidth" {
                json!(slice)
            } else {
                json!(plain)
            })
        }
        Some("u.extractSegments") => Ok(json!(maestro_extensions_wasm::utils::extract_segments(
            api,
            text,
            maestro_extensions_wasm::utils::SegmentSelection {
                before_end: number(&args[1])?,
                after_start: number(&args[2])?,
                after_len: number(&args[3])?,
                strict_after: args[4].as_bool().ok_or("missing strictness")?,
            }
        )?)),
        _ => Err("unknown helper observation".into()),
    }
}
