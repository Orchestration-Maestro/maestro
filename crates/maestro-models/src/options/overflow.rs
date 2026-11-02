//! Classification of provider diagnostics and measured context usage.
use std::{fmt::Write as _, sync::LazyLock};

use regex::RegexSet;

use crate::{AssistantMessage, StopReason};

/// Compiled patterns for provider context-window errors.
static OVERFLOW_PATTERNS: LazyLock<Result<RegexSet, regex::Error>> = LazyLock::new(|| {
    let space = maestro_text::WHITESPACE
        .chars()
        .fold(String::new(), |mut class, c| {
            let _ = write!(class, "\\x{{{:X}}}", u32::from(c));
            class
        });
    let no_body = format!(r"^4(?:00|13)[{space}]*(?:status code)?[{space}]*\(no body\)");
    RegexSet::new(
        [
            "prompt is too long",
            "request_too_large",
            "input is too long for requested model",
            "exceeds the context window",
            r"input token count[^\n\r\x{2028}\x{2029}]*exceeds the maximum",
            r"maximum prompt length is [0-9]+",
            "reduce the length of the messages",
            r"maximum context length is [0-9]+ tokens",
            r"exceeds (?:the )?(?:model'?s )?maximum context length(?: of [0-9,]+ tokens?|[\s]*\([0-9,]+\))",
            r"input \([0-9]+ tokens\) is longer than the model'?s context length \([0-9]+ tokens\)",
            r"exceeds the limit of [0-9]+",
            "exceeds the available context size",
            "greater than the context length",
            "context window exceeds limit",
            "exceeded model token limit",
            r"too large for model with [0-9]+ maximum context length",
            "model_context_window_exceeded",
            r"prompt too long; exceeded (?:max )?context length",
            r"context[_ ]length[_ ]exceeded",
            "too many tokens",
            "token limit exceeded",
        ]
        .into_iter()
        .chain([no_body.as_str()]),
    )
});

/// Compiled exclusions for unrelated provider failures.
static NON_OVERFLOW_PATTERNS: LazyLock<Result<RegexSet, regex::Error>> = LazyLock::new(|| {
    RegexSet::new([
        r"^(throttling error|throttlingexception|service unavailable):",
        "rate limit",
        "too many requests",
    ])
});

/// Detect overflow from diagnostics, successful usage or a filled length stop.
///
/// Error diagnostics match known provider patterns, excluding throttling,
/// rate limits and unavailable services first. A successful stop overflows when
/// input plus cached input exceeds the supplied nonzero context window. A length
/// stop requires zero output and input plus cached input at least 99% of it.
///
/// Providers that silently truncate input without reporting these signals
/// cannot be detected. Custom providers may require checking their diagnostics
/// separately or adding an authored pattern here; no request is sent.
#[must_use]
pub fn is_context_overflow(message: &AssistantMessage, context_window: Option<f64>) -> bool {
    if message.stop_reason == StopReason::Error
        && let Some(text) = &message.error_message
    {
        let text = text.to_ascii_lowercase();
        return NON_OVERFLOW_PATTERNS
            .as_ref()
            .is_ok_and(|patterns| !patterns.is_match(&text))
            && OVERFLOW_PATTERNS
                .as_ref()
                .is_ok_and(|patterns| patterns.is_match(&text));
    }
    let Some(window) = context_window.filter(|window| *window != 0.0) else {
        return false;
    };
    let input = message.usage.input + message.usage.cache_read;
    match message.stop_reason {
        StopReason::Stop => input > window,
        StopReason::Length => message.usage.output == 0.0 && input >= window * 0.99,
        _ => false,
    }
}
