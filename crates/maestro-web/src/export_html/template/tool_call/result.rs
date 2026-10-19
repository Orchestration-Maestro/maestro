//! Reached raw result content display.
use super::invalid;
use serde_json::Value;
use std::{fmt::Write as _, io};

/// Borrow the reached array without reading unrelated content fields.
fn blocks(result: &Value) -> io::Result<&[Value]> {
    result["content"]
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(invalid)
}
/// Join exact text blocks in encounter order, retaining explicit false and zero.
pub(super) fn get_result_text(result: Option<&Value>) -> io::Result<String> {
    let Some(result) = result else {
        return Ok(String::new());
    };
    let mut texts = Vec::new();
    for block in blocks(result)? {
        if block.is_null() {
            return Err(invalid());
        }
        if block["type"] == "text" {
            texts.push(match block.get("text") {
                None | Some(Value::Null) => String::new(),
                Some(Value::String(text)) => text.to_owned(),
                Some(value) => maestro_app::compact_json(value).map_err(io::Error::other)?,
            });
        }
    }
    Ok(texts.join("\n"))
}

/// Borrow strings and canonically display other persisted values without truth filtering.
pub(super) fn display_value(value: &Value) -> io::Result<std::borrow::Cow<'_, str>> {
    if let Value::String(text) = value {
        return Ok(std::borrow::Cow::Borrowed(text));
    }
    maestro_app::compact_json(value)
        .map(std::borrow::Cow::Owned)
        .map_err(io::Error::other)
}

/// Emit ordered image data URLs using attribute-safe escaping at the HTML boundary.
pub(super) fn render_result_images(result: Option<&Value>) -> io::Result<String> {
    let Some(result) = result else {
        return Ok(String::new());
    };
    let mut images = String::new();
    for block in blocks(result)? {
        if block.is_null() {
            return Err(invalid());
        }
        if block["type"] == "image" {
            let mime =
                maestro_app::display_truthy(block.get("mimeType")).map_err(io::Error::other)?;
            let data = maestro_app::display_truthy(block.get("data")).map_err(io::Error::other)?;
            let _ = write!(
                images,
                "<img src=\"data:{};base64,{}\" class=\"tool-image\" />",
                super::html(mime.as_deref().unwrap_or("image/png")),
                super::html(data.as_deref().unwrap_or(""))
            );
        }
    }
    Ok(if images.is_empty() {
        images
    } else {
        format!("<div class=\"tool-images\">{images}</div>")
    })
}
