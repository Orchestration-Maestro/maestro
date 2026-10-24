//! Borrowed display reads and preview normalization.
pub(super) use maestro_app::export_html::template::{
    extract_content_prefix as content, read_text as text,
};
use maestro_app::{is_whitespace_scalar, truncate_utf16};
use maestro_theme::{Theme, ThemeColor};
use serde_json::Value;
use std::io;
/// Normalize display separators, retaining interior carriage returns.
pub(super) fn normalize(text: &str) -> String {
    text.replace(['\n', '\t'], " ")
        .trim_matches(is_whitespace_scalar)
        .to_owned()
}
/// Dispatch entry kinds without decoding unread message fields.
pub(super) fn entry(entry: &Value, theme: &Theme, styles: bool) -> io::Result<String> {
    let fg = |key, body: &str| theme.fg(&key, body).map_err(io::Error::other);
    match entry["type"].as_str() {
        Some("message") => message(&entry["message"], theme),
        Some("custom_message") => {
            if !(entry["content"].is_string() || entry["content"].is_array()) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "custom content is not text or blocks",
                ));
            }
            Ok(fg(
                ThemeColor::CustomMessageLabel,
                &format!("[{}]: ", text(&entry["customType"])?),
            )? + &normalize(&content(&entry["content"], usize::MAX)?))
        }
        Some("compaction") => {
            let tokens = entry["tokensBefore"].as_f64().ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "token count is not numeric")
            })?;
            let rounded = maestro_app::compact_json(&Value::from((tokens / 1000.0).round()))
                .map_err(io::Error::other)?;
            fg(
                ThemeColor::BorderAccent,
                &format!("[compaction: {rounded}k tokens]"),
            )
        }
        Some("branch_summary") => {
            Ok(fg(ThemeColor::Warning, "[branch summary]: ")?
                + &normalize(text(&entry["summary"])?))
        }
        Some("model_change") => fg(
            ThemeColor::Dim,
            &format!("[model: {}]", text(&entry["modelId"])?),
        ),
        Some("thinking_level_change") => fg(
            ThemeColor::Dim,
            &format!("[thinking: {}]", text(&entry["thinkingLevel"])?),
        ),
        Some("custom") => fg(
            ThemeColor::Dim,
            &format!("[custom: {}]", text(&entry["customType"])?),
        ),
        Some("label") => {
            let label = entry
                .get("label")
                .filter(|v| !v.is_null())
                .map_or(Ok("(cleared)"), text)?;
            fg(ThemeColor::Dim, &format!("[label: {label}]"))
        }
        Some("session_info") => {
            let name = if maestro_app::is_truthy(&entry["name"]) {
                fg(ThemeColor::Dim, text(&entry["name"])?)?
            } else {
                Theme::italic(&fg(ThemeColor::Dim, "empty")?, styles)
            };
            Ok(fg(ThemeColor::Dim, "[title: ")? + &name + &fg(ThemeColor::Dim, "]")?)
        }
        _ => Ok(String::new()),
    }
}
/// Message roles preserve their individual label and fallback policies.
fn message(message: &Value, theme: &Theme) -> io::Result<String> {
    let fg = |key, body: &str| theme.fg(&key, body).map_err(io::Error::other);
    match message["role"].as_str() {
        Some("user") => {
            Ok(fg(ThemeColor::Accent, "user: ")? + &normalize(&content(&message["content"], 200)?))
        }
        Some("assistant") => {
            let preview = normalize(&content(&message["content"], 200)?);
            let body = if !preview.is_empty() {
                preview
            } else if message["stopReason"] == "aborted" {
                fg(ThemeColor::Muted, "(aborted)")?
            } else if maestro_app::is_truthy(&message["errorMessage"]) {
                fg(
                    ThemeColor::Error,
                    &truncate_utf16(&normalize(text(&message["errorMessage"])?), 80),
                )?
            } else {
                fg(ThemeColor::Muted, "(no content)")?
            };
            Ok(fg(ThemeColor::Success, "assistant: ")? + &body)
        }
        Some("bashExecution") => {
            let command = message
                .get("command")
                .filter(|v| !v.is_null())
                .map_or(Ok(""), text)?;
            fg(ThemeColor::Dim, &format!("[bash]: {}", normalize(command)))
        }
        _ => fg(ThemeColor::Dim, &format!("[{}]", text(&message["role"])?)),
    }
}
