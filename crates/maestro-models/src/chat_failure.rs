//! Private upstream classification; raw text never escapes.
use crate::Failure;
use serde_json::Value;
pub(crate) fn classify(status: Option<u16>, value: &Value) -> Failure {
    let error = value.get("error").unwrap_or(value);
    let code = error["code"].as_str().unwrap_or("");
    let kind = error["type"].as_str().unwrap_or("");
    let text = error["message"].as_str().unwrap_or("").to_ascii_lowercase();
    if matches!(status, Some(401 | 403))
        || matches!(
            code,
            "invalid_api_key" | "authentication_error" | "unauthorized"
        )
        || kind == "authentication_error"
    {
        return Failure::AuthenticationFailed;
    }
    if matches!(code, "insufficient_quota" | "billing_hard_limit_reached")
        || matches!(
            kind,
            "insufficient_quota" | "quota_exceeded" | "billing_error"
        )
    {
        return Failure::QuotaExceeded;
    }
    if overflow(&text)
        || code == "context_length_exceeded"
        || code == "model_context_window_exceeded"
    {
        return Failure::ContextOverflow;
    }
    if status == Some(429)
        || code == "rate_limit_exceeded"
        || kind == "rate_limit_error"
        || text.starts_with("throttling error:")
        || text.contains("rate limit")
        || text.contains("too many requests")
    {
        return Failure::Throttled;
    }
    if status.is_some_and(|s| s >= 500)
        || kind == "overloaded_error"
        || text.starts_with("service unavailable:")
    {
        return Failure::Overloaded;
    }
    status
        .map(|status| Failure::HttpStatus { status })
        .unwrap_or(Failure::AdapterFailed)
}
fn overflow(text: &str) -> bool {
    if text.starts_with("throttling error:")
        || text.starts_with("service unavailable:")
        || text.contains("rate limit")
        || text.contains("too many requests")
    {
        return false;
    }
    [
        "prompt is too long",
        "request_too_large",
        "input is too long for requested model",
        "exceeds the context window",
        "reduce the length of the messages",
        "exceeds the available context size",
        "greater than the context length",
        "context window exceeds limit",
        "exceeded model token limit",
        "model_context_window_exceeded",
        "prompt too long; exceeded context length",
        "prompt too long; exceeded max context length",
        "context_length_exceeded",
        "context length exceeded",
        "context_length exceeded",
        "context length_exceeded",
        "too many tokens",
        "token limit exceeded",
    ]
    .iter()
    .any(|p| text.contains(p))
        || [
            ("maximum prompt length is ", ""),
            ("maximum context length is ", " tokens"),
            ("exceeds the limit of ", ""),
            ("too large for model with ", " maximum context length"),
        ]
        .iter()
        .any(|(a, b)| numeric(text, a, b))
        || text
            .split(['\n', '\r', '\u{2028}', '\u{2029}'])
            .any(|line| {
                line.find("input token count")
                    .is_some_and(|a| line[a + 17..].contains("exceeds the maximum"))
            })
        || ["400", "413"].iter().any(|prefix| {
            text.strip_prefix(prefix).is_some_and(|rest| {
                let rest = crate::scalar::trim(rest);
                let rest = rest.strip_prefix("status code").unwrap_or(rest);
                crate::scalar::trim(rest).starts_with("(no body)")
            })
        })
}

fn numeric(text: &str, prefix: &str, suffix: &str) -> bool {
    for (position, _) in text.match_indices(prefix) {
        let rest = &text[position + prefix.len()..];
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        if digits > 0 && rest[digits..].starts_with(suffix) {
            return true;
        }
    }
    false
}
