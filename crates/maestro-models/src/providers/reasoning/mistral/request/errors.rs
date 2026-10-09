//! Native transport context and provider diagnostics.

use super::trim;
use crate::providers::http::{FetchError, RequestFailure};
use std::fmt::Write as _;

/// Format only nonempty trimmed HTTP bodies with scalar-safe clipping.
pub(in crate::providers::reasoning::mistral) fn format_error(
    status: Option<u16>,
    body: Option<&str>,
    message: &str,
) -> String {
    let Some(status) = status else {
        return message.to_owned();
    };
    let body = body.map(trim).filter(|text| !text.is_empty());
    let detail = body.map_or_else(
        || message.to_owned(),
        |text| {
            let length = text.chars().count();
            if length <= 4000 {
                text.to_owned()
            } else {
                format!(
                    "{}... [truncated {} chars]",
                    text.chars().take(4000).collect::<String>(),
                    length - 4000
                )
            }
        },
    );
    format!("Mistral API error ({status}): {detail}")
}

/// Preserve native causes while identifying setup failures.
pub(super) fn setup_failure(error: FetchError) -> RequestFailure {
    RequestFailure::new(match error {
        FetchError::Connection(cause) => format!("Unable to make request: {}", cause.message),
        FetchError::Timeout => "Request timed out: Request timed out.".into(),
        FetchError::Aborted => "Request aborted by client: Request was aborted.".into(),
    })
}

/// Construct the dependency's empty-body fallback before provider formatting.
pub(super) fn fallback(
    status: u16,
    content_type: Option<&str>,
    body: &str,
    matched: bool,
) -> String {
    let reason = if matched {
        "API error occurred"
    } else {
        "Unexpected Status or Content-Type"
    };
    let mut message = format!("{reason}: Status {status}");
    let content_type = content_type
        .filter(|text| !text.is_empty())
        .unwrap_or("\"\"");
    if content_type != "application/json" {
        message.push_str(" Content-Type ");
        if content_type.contains(' ') {
            message.push('"');
            message.push_str(content_type);
            message.push('"');
        } else {
            message.push_str(content_type);
        }
    }
    let body = if body.is_empty() { "\"\"" } else { body };
    let count = body.chars().count();
    message.push_str(if count > 100 { "\nBody: " } else { ". Body: " });
    message.extend(body.chars().take(10_000));
    if count > 10_000 {
        let _ = write!(message, "...and {} more chars", count - 10_000);
    }
    trim(&message).to_owned()
}
