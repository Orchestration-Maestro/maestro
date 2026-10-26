//! Ordinary component helper operands delegate to their existing domain owners.
use serde::{Serialize, de::DeserializeOwned};

/// Dispatches a synchronous helper without retaining any operands.
///
/// # Errors
/// Returns malformed operands or the native owner's failure unchanged.
pub fn call(operation: &str, data: &str) -> Result<String, String> {
    match operation {
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
        _ => Err(format!("Unknown native helper: {operation}")),
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
