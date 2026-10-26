//! Complete tool-output accounting observed by an unchanged compiled author.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    ExtensionAPI, ExtensionResult,
    tools::truncate::{
        DEFAULT_MAX_BYTES, DEFAULT_MAX_LINES, GREP_MAX_LINE_LENGTH, TruncationOptions, format_size,
        truncate_head, truncate_line, truncate_tail,
    },
};
use serde_json::{Value, json};
/// Reads one output observation through its typed facade.
pub(super) fn observe(api: &ExtensionAPI, request: &Value) -> ExtensionResult<Value> {
    let args = &request["args"];
    match request["operation"].as_str() {
        Some(operation @ ("tr.truncateHead" | "tr.truncateTail")) => {
            let options = TruncationOptions {
                max_lines: args[1]["maxLines"]
                    .as_u64()
                    .map(usize::try_from)
                    .transpose()
                    .map_err(|error| error.to_string())?,
                max_bytes: args[1]["maxBytes"]
                    .as_u64()
                    .map(usize::try_from)
                    .transpose()
                    .map_err(|error| error.to_string())?,
            };
            let content = args[0].as_str().ok_or("missing content")?;
            Ok(if operation == "tr.truncateHead" {
                json!(truncate_head(api, content, options)?)
            } else {
                json!(truncate_tail(api, content, options)?)
            })
        }
        Some("tr.truncateLine") => {
            let limit = args[1]
                .as_u64()
                .map(usize::try_from)
                .transpose()
                .map_err(|error| error.to_string())?;
            Ok(json!(truncate_line(
                api,
                args[0].as_str().ok_or("missing line")?,
                limit
            )?))
        }
        Some("tr.formatSize") => Ok(json!(format_size(api, byte_count(&args[0])?)?)),
        Some("tr.constants") => {
            let head = truncate_head(api, "", TruncationOptions::default())?;
            let line = truncate_line(api, &"x".repeat(501), None)?;
            let grep = line
                .text
                .strip_suffix("... [truncated]")
                .ok_or("missing truncation marker")?
                .chars()
                .count();
            if head.max_lines.total_cmp(&2000.0).is_ne()
                || head.max_bytes.total_cmp(&51200.0).is_ne()
                || grep != GREP_MAX_LINE_LENGTH
                || DEFAULT_MAX_LINES != 2000
                || DEFAULT_MAX_BYTES != 51200
            {
                return Err("native truncation defaults changed".into());
            }
            Ok(json!({"lines":DEFAULT_MAX_LINES,"bytes":DEFAULT_MAX_BYTES,"grep":grep}))
        }
        _ => Err("unknown helper observation".into()),
    }
}

/// Supplies exceptional floating-point arguments without encoding them as JSON numbers.
fn byte_count(value: &Value) -> ExtensionResult<f64> {
    match value["$number"].as_str() {
        Some("NaN") => Ok(f64::NAN),
        Some("Infinity") => Ok(f64::INFINITY),
        Some("-Infinity") => Ok(f64::NEG_INFINITY),
        _ => value.as_f64().ok_or_else(|| "missing byte count".into()),
    }
}
