use crate::{AssistantMessage, StopReason};

/// Check if an assistant message represents a context overflow error.
///
/// This handles two cases:
/// 1. Error-based overflow: Most providers return stopReason "error" with a
///    specific error message pattern.
/// 2. Silent overflow: Some providers accept overflow requests and return
///    successfully. For these, we check if usage.input exceeds the context window.
///
/// ## Reliability by Provider
///
/// **Reliable detection (returns error with detectable message):**
/// - Anthropic: "prompt is too long: X tokens > Y maximum" or "request_too_large"
/// - OpenAI (Completions & Responses): "exceeds the context window"
/// - Google Gemini: "input token count exceeds the maximum"
/// - xAI (Grok): "maximum prompt length is X but request contains Y"
/// - Groq: "reduce the length of the messages"
/// - Cerebras: 400/413 status code (no body)
/// - Mistral: "Prompt contains X tokens ... too large for model with Y maximum context length"
/// - OpenRouter (all backends): "maximum context length is X tokens"
/// - llama.cpp: "exceeds the available context size"
/// - LM Studio: "greater than the context length"
/// - Kimi For Coding: "exceeded model token limit: X (requested: Y)"
///
/// **Unreliable detection:**
/// - z.ai: Sometimes accepts overflow silently (detectable via usage.input > contextWindow),
///   sometimes returns rate limit errors. Pass contextWindow param to detect silent overflow.
/// - Xiaomi MiMo: Truncates input to fit contextWindow then returns stopReason "length" with
///   output=0. Pass contextWindow param to detect via the "filled context + zero output" signal.
/// - Ollama: May truncate input silently for some setups, but may also return explicit
///   overflow errors that match the patterns above. Silent truncation still cannot be
///   detected here because we do not know the expected token count.
///
/// ## Custom Providers
///
/// If you've added custom models via settings.json, this function may not detect
/// overflow errors from those providers. To add support:
///
/// 1. Send a request that exceeds the model's context window
/// 2. Check the errorMessage in the response
/// 3. Create a regex pattern that matches the error
/// 4. The pattern should be added to OVERFLOW_PATTERNS in this file, or
///    check the errorMessage yourself before calling this function
///
/// @param message - The assistant message to check
/// @param context_window - Optional context window size for detecting silent overflow (z.ai)
/// @returns true if the message indicates a context overflow
pub fn is_context_overflow(message: &AssistantMessage, context_window: Option<f64>) -> bool {
    if message.stop_reason == StopReason::Error
        && let Some(text) = message.error_message.as_deref().filter(|s| !s.is_empty())
        && !non_overflow_patterns().iter().any(|p| p.test(text))
        && overflow_patterns().iter().any(|p| p.test(text))
    {
        return true;
    }
    if let Some(window) = context_window.filter(|w| *w != 0.0 && !w.is_nan())
        && message.stop_reason == StopReason::Stop
        && message.usage.input + message.usage.cache_read > window
    {
        return true;
    }
    if let Some(window) = context_window.filter(|w| *w != 0.0 && !w.is_nan())
        && message.stop_reason == StopReason::Length
        && message.usage.output == 0.0
        && message.usage.input + message.usage.cache_read >= window * 0.99
    {
        return true;
    }
    false
}

/// Regex patterns to detect context overflow errors from different providers.
///
/// These patterns match error messages returned when the input exceeds
/// the model's context window.
///
/// Provider-specific patterns (with example error messages):
///
/// - Anthropic: "prompt is too long: 213462 tokens > 200000 maximum"
/// - Anthropic: "413 {\"error\":{\"type\":\"request_too_large\",\"message\":\"Request exceeds the maximum size\"}}"
/// - OpenAI: "Your input exceeds the context window of this model"
/// - Google: "The input token count (1196265) exceeds the maximum number of tokens allowed (1048575)"
/// - xAI: "This model's maximum prompt length is 131072 but the request contains 537812 tokens"
/// - Groq: "Please reduce the length of the messages or completion"
/// - OpenRouter: "This endpoint's maximum context length is X tokens. However, you requested about Y tokens"
/// - llama.cpp: "the request exceeds the available context size, try increasing it"
/// - LM Studio: "tokens to keep from the initial prompt is greater than the context length"
/// - GitHub Copilot: "prompt token count of X exceeds the limit of Y"
/// - MiniMax: "invalid params, context window exceeds limit"
/// - Kimi For Coding: "Your request exceeded model token limit: X (requested: Y)"
/// - Cerebras: "400/413 status code (no body)"
/// - Mistral: "Prompt contains X tokens ... too large for model with Y maximum context length"
/// - z.ai: Does NOT error, accepts overflow silently - handled via usage.input > contextWindow
/// - Xiaomi MiMo: Truncates input to fill contextWindow exactly, then returns finish_reason "length"
///   with output=0 (no room left to generate). Detected via stopReason "length" + zero output +
///   input filling the context window.
/// - Ollama: Some deployments truncate silently, others return errors like "prompt too long; exceeded max context length by X tokens"
static OVERFLOW_PATTERNS: std::sync::OnceLock<Vec<RegExp>> = std::sync::OnceLock::new();

/// An opaque, stateless case-insensitive overflow pattern.
#[derive(Clone)]
pub struct RegExp {
    source: &'static str,
    regex: regress::Regex,
}
impl RegExp {
    /// Original pattern source.
    pub fn source(&self) -> &str {
        self.source
    }
    /// Original matching flags.
    pub fn flags(&self) -> &str {
        "i"
    }
    /// Search the supplied text without retaining matching state.
    pub fn test(&self, text: &str) -> bool {
        let units: Vec<u16> = text.encode_utf16().collect();
        self.regex.find_from_utf16(&units, 0).next().is_some()
    }
}
fn pattern(source: &'static str) -> RegExp {
    RegExp {
        source,
        regex: regress::Regex::with_flags(source, "i").expect("valid owned overflow pattern"),
    }
}
fn overflow_patterns() -> &'static [RegExp] {
    OVERFLOW_PATTERNS.get_or_init(|| {
        [
            r"prompt is too long",
            r"request_too_large",
            r"input is too long for requested model",
            r"exceeds the context window",
            r"input token count.*exceeds the maximum",
            r"maximum prompt length is \d+",
            r"reduce the length of the messages",
            r"maximum context length is \d+ tokens",
            r"exceeds the limit of \d+",
            r"exceeds the available context size",
            r"greater than the context length",
            r"context window exceeds limit",
            r"exceeded model token limit",
            r"too large for model with \d+ maximum context length",
            r"model_context_window_exceeded",
            r"prompt too long; exceeded (?:max )?context length",
            r"context[_ ]length[_ ]exceeded",
            r"too many tokens",
            r"token limit exceeded",
            r"^4(?:00|13)\s*(?:status code)?\s*\(no body\)",
        ]
        .into_iter()
        .map(pattern)
        .collect()
    })
}
/// Get the overflow patterns for testing purposes.
pub fn get_overflow_patterns() -> Vec<RegExp> {
    overflow_patterns().to_vec()
}

/// Patterns that indicate non-overflow errors (e.g. rate limiting, server errors).
/// Error messages matching any of these are excluded from overflow detection
/// even if they also match an OVERFLOW_PATTERN.
///
/// Example: Bedrock formats throttling errors as "ThrottlingException: Too many tokens,
/// please wait before trying again." which would match the /too many tokens/i overflow
/// pattern without this exclusion.
static NON_OVERFLOW_PATTERNS: std::sync::OnceLock<Vec<RegExp>> = std::sync::OnceLock::new();
fn non_overflow_patterns() -> &'static [RegExp] {
    NON_OVERFLOW_PATTERNS.get_or_init(|| {
        [
            r"^(Throttling error|Service unavailable):",
            r"rate limit",
            r"too many requests",
        ]
        .into_iter()
        .map(pattern)
        .collect()
    })
}
