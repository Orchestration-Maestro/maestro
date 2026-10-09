//! Response-session setup failure policy.

use super::request::diagnostic;
use crate::DiagnosticErrorInfo;
use regex::Regex;
use std::sync::LazyLock;

/// ASCII literals use non-Unicode case matching; the optional separator is one BMP non-line unit.
static RETRYABLE: LazyLock<Result<Regex, regex::Error>> = LazyLock::new(|| {
    let separator = r"[^\n\r\u{2028}\u{2029}\x{10000}-\x{10FFFF}]?";
    Regex::new(&format!(
        r"(?i-u:rate){separator}(?i-u:limit)|(?i-u:overloaded)|(?i-u:service){separator}(?i-u:unavailable)|(?i-u:upstream){separator}(?i-u:connect)|(?i-u:connection){separator}(?i-u:refused)"
    ))
});

/// Selected statuses always retry; other statuses depend on the authored body pattern.
pub(super) fn is_retryable_error(status: u16, text: &str) -> Result<bool, DiagnosticErrorInfo> {
    if matches!(status, 429 | 500 | 502 | 503 | 504) {
        return Ok(true);
    }
    RETRYABLE
        .as_ref()
        .map(|expression| expression.is_match(text))
        .map_err(|error| diagnostic(error.to_string()))
}

use crate::providers::json_text::{is_truthy, member, raw_json, raw_number};
use serde_json::value::RawValue;

/// Usage codes have the source's non-Unicode case-insensitive matching.
static USAGE_LIMIT: LazyLock<Result<Regex, regex::Error>> = LazyLock::new(|| {
    Regex::new(r"(?i-u:usage_limit_reached|usage_not_included|rate_limit_exceeded)")
});

/// Selected server text and optional friendly usage text.
pub(super) struct ErrorResponse {
    /// Server message, raw body, status text or fixed fallback.
    pub(super) message: String,
    /// Usage-limit rendering, preferred by invocation.
    pub(super) friendly_message: Option<String>,
}

/// Read a truthy declared string; a wrong type follows the local envelope catch.
fn field(raw: &RawValue, name: &str) -> Result<Option<String>, serde_json::Error> {
    member(raw, name)
        .filter(|value| is_truthy(value))
        .map(|value| serde_json::from_str(value.get()))
        .transpose()
}

/// Read the clock only after a selected usage code and a nonzero reset.
fn friendly(error: &RawValue, now: impl FnOnce() -> f64) -> Result<String, serde_json::Error> {
    let plan = field(error, "plan_type")?.map_or_else(String::new, |plan| {
        format!(" ({} plan)", plan.to_lowercase())
    });
    let when = member(error, "resets_at")
        .and_then(raw_number)
        .filter(|reset| *reset != 0.0)
        .map_or_else(String::new, |reset| {
            let minutes = ((reset * 1000.0 - now()) / 60000.0).round().max(0.0);
            format!(
                " Try again in ~{} min.",
                ryu_js::Buffer::new().format(minutes)
            )
        });
    Ok(format!(
        "You have hit your ChatGPT usage limit{plan}.{when}"
    ))
}

/// Keep the envelope's local catch and the raw/status fallback.
pub(super) fn parse_error_response(
    status: u16,
    raw: &str,
    status_text: &str,
    now: impl FnOnce() -> f64,
) -> ErrorResponse {
    let fallback = if !raw.is_empty() {
        raw
    } else if !status_text.is_empty() {
        status_text
    } else {
        "Request failed"
    };
    let selected = (|| {
        let parsed = raw_json(raw)?;
        let Some(error) = member(parsed, "error").filter(|error| is_truthy(error)) else {
            return Ok(None);
        };
        let code = match field(error, "code")? {
            Some(code) => code,
            None => field(error, "type")?.unwrap_or_default(),
        };
        let usage = status == 429
            || USAGE_LIMIT
                .as_ref()
                .is_ok_and(|pattern| pattern.is_match(&code));
        let friendly_message = if usage {
            Some(friendly(error, now)?)
        } else {
            None
        };
        let message = field(error, "message")?
            .or_else(|| friendly_message.clone())
            .unwrap_or_else(|| fallback.to_owned());
        Ok::<_, serde_json::Error>(Some(ErrorResponse {
            message,
            friendly_message,
        }))
    })();
    selected.ok().flatten().unwrap_or_else(|| ErrorResponse {
        message: fallback.to_owned(),
        friendly_message: None,
    })
}
