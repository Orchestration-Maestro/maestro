#[cfg(test)]
mod tests {
    use maestro_models::{AssistantMessage, StopReason, is_context_overflow};

    fn message(text: Option<&str>) -> AssistantMessage {
        serde_json::from_value(serde_json::json!({
        "role": "assistant", "content": [], "api": "fixture", "provider": "fixture", "model": "controlled",
        "usage": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0, "totalTokens": 0,
            "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0, "total": 0}},
        "stopReason": "error", "errorMessage": text, "timestamp": 1
    })).unwrap()
    }

    #[test]
    fn maestro_overflow_distinguishes_provider_errors() {
        for text in [
            "prompt is too long: 213462 tokens > 200000 maximum",
            "413 {\"error\":{\"type\":\"request_too_large\",\"message\":\"Request exceeds the maximum size\"}}",
            "input is too long for requested model",
            "Your input exceeds the context window of this model",
            "The input token count (1196265) exceeds the maximum number of tokens allowed (1048575)",
            "This model's maximum prompt length is 131072 but the request contains 537812 tokens",
            "Please reduce the length of the messages or completion",
            "This endpoint's maximum context length is 100 tokens. However, you requested about 101 tokens",
            "prompt token count of 200001 exceeds the limit of 200000",
            "the request exceeds the available context size, try increasing it",
            "tokens to keep from the initial prompt is greater than the context length",
            "invalid params, context window exceeds limit",
            "Your request exceeded model token limit: 100 (requested: 101)",
            "Prompt contains 101 tokens ... too large for model with 100 maximum context length",
            "model_context_window_exceeded",
            "400 `prompt too long; exceeded max context length by 100918 tokens`",
            "prompt too long; exceeded context length",
            "context_length_exceeded",
            "context length_exceeded",
            "context_length exceeded",
            "context length exceeded",
            "Too many tokens",
            "token limit exceeded",
            "400 (no body)",
            "413 status code (no body)",
        ] {
            assert!(is_context_overflow(&message(Some(text)), None), "{text}");
        }
        for text in [
            "500 `model runner crashed unexpectedly`",
            "Service unavailable: The service is temporarily unavailable.",
            "Rate limit exceeded, please retry after 30 seconds.",
            "Too many requests. Please slow down.",
            "Service unavailable: prompt is too long",
            "rate limit: token limit exceeded",
            "too many requests: Too many tokens",
        ] {
            assert!(!is_context_overflow(&message(Some(text)), None), "{text}");
        }
    }

    #[test]
    fn maestro_throttling_prefixes_override_token_patterns() {
        for text in [
            "Throttling error: Too many tokens, please wait before trying again.",
            "ThrottlingException: Too many tokens, please wait before trying again.",
            "THROTTLINGEXCEPTION: PROMPT IS TOO LONG",
            "throttling error: token limit exceeded",
        ] {
            assert!(!is_context_overflow(&message(Some(text)), None), "{text}");
        }
        for text in [
            "Too many tokens",
            "prefix ThrottlingException: Too many tokens",
            " Throttling error: Too many tokens",
        ] {
            assert!(is_context_overflow(&message(Some(text)), None), "{text}");
        }
    }

    #[test]
    fn maestro_overflow_requires_error_stop_for_diagnostics() {
        for text in [None, Some(""), Some("unknown failure")] {
            let mut response = message(text);
            response.usage.input = 1000.0;
            assert!(!is_context_overflow(&response, Some(10.0)));
        }
        assert!(is_context_overflow(
            &message(Some("prompt is too long")),
            None
        ));
        for reason in [
            StopReason::Stop,
            StopReason::Length,
            StopReason::ToolUse,
            StopReason::Aborted,
        ] {
            let mut response = message(Some("prompt is too long"));
            response.stop_reason = reason;
            assert!(!is_context_overflow(&response, Some(100.0)));
        }
    }

    #[test]
    fn maestro_overflow_counts_only_context_input() {
        let mut response = message(None);
        response.stop_reason = StopReason::Stop;
        response.usage.cache_read = 10.0;
        for (input, expected) in [(89.0, false), (90.0, false), (91.0, true)] {
            response.usage.input = input;
            assert_eq!(is_context_overflow(&response, Some(100.0)), expected);
        }
        response.stop_reason = StopReason::Length;
        for (input, expected) in [(88.0, false), (89.0, true), (90.0, true)] {
            response.usage.input = input;
            assert_eq!(is_context_overflow(&response, Some(100.0)), expected);
        }
        response.usage.input = 58.0;
        response.usage.cache_read = 1_048_512.0;
        assert!(is_context_overflow(&response, Some(1_048_576.0)));
        response.usage.output = 4096.0;
        assert!(!is_context_overflow(&response, Some(1_048_576.0)));
        response.usage.output = 0.0;
        response.usage.cache_read = 0.0;
        response.usage.input = 100.0;
        assert!(!is_context_overflow(&response, Some(200_000.0)));
        for reason in [StopReason::Stop, StopReason::Length] {
            response.stop_reason = reason;
            assert!(!is_context_overflow(&response, None));
            assert!(!is_context_overflow(&response, Some(0.0)));
            assert!(is_context_overflow(&response, Some(-1.0)));
            response.usage.input = 0.0;
            response.usage.cache_write = 1_000_000.0;
            response.usage.total_tokens = 1_000_000.0;
            response.usage.cost.total = 1_000_000.0;
            assert!(!is_context_overflow(&response, Some(100.0)));
        }
        response.usage.input = 1_000_000.0;
        for reason in [StopReason::ToolUse, StopReason::Aborted, StopReason::Error] {
            response.stop_reason = reason;
            assert!(!is_context_overflow(&response, Some(100.0)));
        }
    }

    #[test]
    fn maestro_overflow_preserves_authored_text_boundaries() {
        for text in [
            "PROMPT IS TOO LONG",
            "MAXIMUM CONTEXT LENGTH IS 123 TOKENS",
            "Input token count abc exceeds the maximum",
            "400(no body)tail",
            "413status code(no body)",
        ] {
            assert!(is_context_overflow(&message(Some(text)), None), "{text}");
        }
        for text in [
            "prompt iſ too long",
            "too many to\u{212A}ens",
            "maximum prompt length is ١٢",
            "maximum context length is １２ tokens",
            "exceeds the limit of ١",
            "too large for model with １２ maximum context length",
            "prefix 400 (no body)",
        ] {
            assert!(!is_context_overflow(&message(Some(text)), None), "{text}");
        }
        for separator in ['\n', '\r', '\u{2028}', '\u{2029}'] {
            let text = format!("input token count{separator}exceeds the maximum");
            assert!(
                !is_context_overflow(&message(Some(&text)), None),
                "{text:?}"
            );
        }
        for separator in [
            '\t', '\n', '\u{000B}', '\u{000C}', '\r', ' ', '\u{00A0}', '\u{1680}', '\u{2000}',
            '\u{200A}', '\u{2028}', '\u{2029}', '\u{202F}', '\u{205F}', '\u{3000}', '\u{FEFF}',
        ] {
            for status in ["400", "413"] {
                let text = format!("{status}{separator}status code{separator}(no body) trailing");
                assert!(is_context_overflow(&message(Some(&text)), None), "{text:?}");
            }
        }
        for separator in ['\u{0085}', '\u{200B}'] {
            for text in [
                format!("400{separator}(no body)"),
                format!("413 status code{separator}(no body)"),
            ] {
                assert!(
                    !is_context_overflow(&message(Some(&text)), None),
                    "{text:?}"
                );
            }
        }
    }
}
