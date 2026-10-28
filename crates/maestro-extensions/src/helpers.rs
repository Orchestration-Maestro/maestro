//! Ordinary component helper operands delegate to their existing domain owners.
use serde::{Serialize, de::DeserializeOwned};
mod conversation;
mod fuzzy;
mod keys;
mod paths;
pub use paths::guest_path;
mod time;
mod tools;

/// Dispatches a synchronous helper without retaining any operands.
///
/// # Errors
/// Returns malformed operands or the native owner's failure unchanged.
pub fn call(operation: &str, data: &str) -> Result<String, String> {
    if operation.starts_with("conversation.") {
        return conversation::call(operation, data);
    }
    if operation.starts_with("paths.") {
        return paths::call(operation, data);
    }
    if operation.starts_with("tools.") {
        return tools::call(operation, data);
    }
    if operation.starts_with("fuzzy.") {
        return fuzzy::call(operation, data);
    }
    if operation.starts_with("keys.") {
        return keys::call(operation, data);
    }
    match operation {
        "time.local_date_time_string" => time::local_date_time_string(data),
        "visible_width" => {
            let (text,): (String,) = decode(data)?;
            encode(&maestro_tui::visible_width(&text))
        }
        "normalize_terminal_output" => {
            let (text,): (String,) = decode(data)?;
            encode(&maestro_tui::normalize_terminal_output(&text))
        }
        "extract_ansi_code" => {
            let (text, pos): (String, usize) = decode(data)?;
            encode(
                &maestro_tui::extract_ansi_code(&text, pos)
                    .map(|ansi| serde_json::json!({"code":ansi.code,"length":ansi.length})),
            )
        }
        "is_whitespace_char" => {
            let (text,): (String,) = decode(data)?;
            encode(&maestro_tui::is_whitespace_char(&text))
        }
        "is_punctuation_char" => {
            let (text,): (String,) = decode(data)?;
            encode(&maestro_tui::is_punctuation_char(&text))
        }
        _ => columns(operation, data),
    }
}

/// Decodes only the operands of the selected operation.
fn decode<T: DeserializeOwned>(data: &str) -> Result<T, String> {
    serde_json::from_str(data).map_err(|error| error.to_string())
}

/// Encodes ordinary helper results; complete metadata uses its typed value boundary.
fn encode<T: Serialize>(value: &T) -> Result<String, String> {
    serde_json::to_string(value).map_err(|error| error.to_string())
}

/// Delegates column operations to the same native terminal owner.
fn columns(operation: &str, data: &str) -> Result<String, String> {
    match operation {
        "wrap_text_with_ansi" => {
            let (text, width): (String, usize) = decode(data)?;
            encode(&maestro_tui::wrap_text_with_ansi(&text, width))
        }
        "apply_background_to_line" => {
            let (line, width): (String, usize) = decode(data)?;
            encode(&maestro_tui::apply_background_to_line(
                &line,
                width,
                str::to_owned,
            ))
        }
        "truncate_to_width" => {
            let (text, width, ellipsis, pad): (String, usize, String, bool) = decode(data)?;
            encode(&maestro_tui::truncate_to_width(
                &text,
                width,
                maestro_tui::TruncateOptions {
                    ellipsis: &ellipsis,
                    pad,
                },
            ))
        }
        "slice_with_width" => {
            let (line, start, length, strict): (String, usize, usize, bool) = decode(data)?;
            let slice = maestro_tui::slice_with_width(&line, start, length, strict);
            encode(&serde_json::json!({"text":slice.text,"width":slice.width}))
        }
        "extract_segments" => {
            let (line, before_end, after_start, after_len, strict): (
                String,
                usize,
                usize,
                usize,
                bool,
            ) = decode(data)?;
            let segments =
                maestro_tui::extract_segments(&line, before_end, after_start, after_len, strict);
            encode(
                &serde_json::json!({"before":segments.before,"beforeWidth":segments.before_width,"after":segments.after,"afterWidth":segments.after_width}),
            )
        }
        "strip_frontmatter" => {
            let (text,): (String,) = decode(data)?;
            encode(
                &maestro_resources::frontmatter::strip_frontmatter(&text)
                    .map_err(|error| error.to_string())?,
            )
        }
        _ => Err(format!("Unknown native helper: {operation}")),
    }
}

/// Complete native metadata model, not a JSON number projection.
pub use maestro_resources::frontmatter::{FrontmatterValue, parse_frontmatter, parse_yaml};

/// Native size formatting over the complete floating-point argument domain.
pub use maestro_tools::format_size;
