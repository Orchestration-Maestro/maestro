//! Shared content reads with explicit complete and prefix policies.
use maestro_json::is_truthy;
use maestro_text::{is_whitespace_scalar, truncate_utf16};
use serde_json::Value;
use std::io;
/// Borrows text at its consuming read site.
///
/// # Errors
/// Returns invalid data when the reached value is not a string.
pub fn read_text(value: &Value) -> io::Result<&str> {
    value
        .as_str()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "display field is not text"))
}
/// Concatenates complete text blocks without separators or normalization.
///
/// # Errors
/// Returns invalid data for a reached null block or truthy nonstring text.
pub fn extract_content(value: &Value) -> io::Result<String> {
    collect_content(value, usize::MAX, true)
}
/// Prefix of concatenated text, stopping before later unread blocks.
///
/// # Errors
/// Returns invalid data for a selected nonstring text block before the cutoff.
pub fn extract_content_prefix(value: &Value, limit: usize) -> io::Result<String> {
    collect_content(value, limit, false)
}
/// Collect using the caller's complete-content or terminal-prefix policy.
fn collect_content(value: &Value, limit: usize, complete: bool) -> io::Result<String> {
    if let Some(text) = value.as_str() {
        return Ok(truncate_utf16(text, limit));
    }
    let mut result = String::new();
    let mut units = 0;
    for block in value.as_array().into_iter().flatten() {
        if complete && block.is_null() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "null content block",
            ));
        }
        if block["type"] != "text" || complete && !is_truthy(&block["text"]) {
            continue;
        }
        let text = read_text(&block["text"])?;
        result.push_str(text);
        if limit != usize::MAX {
            units += text.encode_utf16().count();
        }
        if limit != usize::MAX && units >= limit {
            break;
        }
    }
    if limit == usize::MAX {
        Ok(result)
    } else {
        Ok(truncate_utf16(&result, limit))
    }
}
/// Tests string and array content, stopping before unread later malformed text.
///
/// # Errors
/// Returns invalid data for a reached truthy nonstring text block.
pub fn has_text_content(content: &Value) -> io::Result<bool> {
    if let Some(text) = content.as_str() {
        return Ok(text.chars().any(|scalar| !is_whitespace_scalar(scalar)));
    }
    if let Some(blocks) = content.as_array() {
        for block in blocks
            .iter()
            .filter(|block| block.get("type").is_some_and(|kind| kind == "text"))
        {
            let text = &block["text"];
            if !is_truthy(text) {
                continue;
            }
            let text = text.as_str().ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "text block is not a string")
            })?;
            if text.chars().any(|scalar| !is_whitespace_scalar(scalar)) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}
