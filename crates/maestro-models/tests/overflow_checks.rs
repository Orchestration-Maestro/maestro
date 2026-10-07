use maestro_models::*;

fn message(reason: StopReason, input: f64, cache_read: f64, output: f64) -> AssistantMessage {
    AssistantMessage {
        content: vec![],
        api: "openai-completions".into(),
        provider: "xiaomi".into(),
        model: "mimo-v2.5-pro".into(),
        response_model: None,
        response_id: None,
        diagnostics: None,
        usage: Usage {
            input,
            cache_read,
            output,
            cache_write: 0.0,
            total_tokens: input + cache_read + output,
            cost: UsageCost {
                input: 0.0,
                output: 0.0,
                cache_read: 0.0,
                cache_write: 0.0,
                total: 0.0,
            },
        },
        stop_reason: reason,
        error_message: None,
        timestamp: 0.0,
    }
}
fn error(text: &str) -> AssistantMessage {
    let mut result = message(StopReason::Error, 0.0, 0.0, 0.0);
    result.provider = "ollama".into();
    result.model = "qwen3.5:35b".into();
    result.error_message = Some(text.into());
    result
}

#[test]
fn ollama_explicit_prompt_overflow_is_detected() {
    assert!(is_context_overflow(
        &error("400 `prompt too long; exceeded max context length by 100918 tokens`"),
        Some(32768.0)
    ));
}

#[test]
fn ollama_runner_failure_is_not_overflow() {
    assert!(!is_context_overflow(
        &error("500 `model runner crashed unexpectedly`"),
        Some(32768.0)
    ));
}

#[test]
fn overflow_patterns_preserve_sources_flags_order_and_copy() {
    let sources = [
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
    ];
    let mut patterns = get_overflow_patterns();
    assert_eq!(
        patterns.iter().map(|p| p.source()).collect::<Vec<_>>(),
        sources
    );
    for (index, pattern) in patterns.iter().enumerate() {
        assert_eq!(pattern.flags(), "i");
        let copied = pattern.clone();
        assert_eq!(copied.source(), pattern.source());
        assert_eq!(copied.flags(), pattern.flags());
        for (text, expected) in [
            ("prompt is too long", index == 0),
            ("413 (no body)", index == 19),
            ("unrelated", false),
        ] {
            assert_eq!(copied.test(text), expected);
            assert_eq!(pattern.test(text), expected);
            assert_eq!(pattern.test(text), expected);
        }
    }
    patterns.reverse();
    patterns.pop();
    patterns.clear();
    assert_eq!(
        get_overflow_patterns()
            .iter()
            .map(|p| p.source())
            .collect::<Vec<_>>(),
        sources
    );
}

#[test]
fn overflow_patterns_recognize_each_listed_message() {
    let fixtures = [
        r#"prompt is too long: 213462 tokens > 200000 maximum"#,
        r#"413 {"error":{"type":"request_too_large","message":"Request exceeds the maximum size"}}"#,
        r#"Input is too long for requested model"#,
        r#"Your input exceeds the context window of this model"#,
        r#"The input token count (1196265) exceeds the maximum number of tokens allowed (1048575)"#,
        r#"This model's maximum prompt length is 131072 but the request contains 537812 tokens"#,
        r#"Please reduce the length of the messages or completion"#,
        r#"This endpoint's maximum context length is 8192 tokens. However, you requested about 9000 tokens"#,
        r#"prompt token count of 9000 exceeds the limit of 8192"#,
        r#"the request exceeds the available context size, try increasing it"#,
        r#"tokens to keep from the initial prompt is greater than the context length"#,
        r#"invalid params, context window exceeds limit"#,
        r#"Your request exceeded model token limit: 8192 (requested: 9000)"#,
        r#"Prompt contains 9000 tokens ... too large for model with 8192 maximum context length"#,
        r#"model_context_window_exceeded"#,
        r#"prompt too long; exceeded max context length by 100918 tokens"#,
        r#"context_length_exceeded"#,
        r#"Too many tokens"#,
        r#"token limit exceeded"#,
        r#"413 status code (no body)"#,
    ];
    for (index, (pattern, text)) in get_overflow_patterns().iter().zip(fixtures).enumerate() {
        for variant in [
            text.to_owned(),
            text.to_uppercase(),
            format!("{text} suffix"),
        ] {
            assert!(pattern.test(&variant), "{index}: {variant}");
            assert!(pattern.test(&variant));
            assert!(
                is_context_overflow(&error(&variant), None),
                "{index}: {variant}"
            );
        }
        if index < 19 {
            let prefixed = format!("prefix {text} suffix");
            assert!(pattern.test(&prefixed));
            assert!(is_context_overflow(&error(&prefixed), None));
        }
    }
}

#[test]
fn overflow_exclusions_take_precedence_over_matches() {
    for prefix in ["Throttling error:", "Service unavailable:"] {
        for text in [
            format!("{prefix} too many tokens"),
            format!("{prefix} too many tokens").to_uppercase(),
        ] {
            assert!(!is_context_overflow(&error(&text), Some(1.0)));
        }
        assert!(is_context_overflow(
            &error(&format!("embedded {prefix} too many tokens")),
            None
        ));
    }
    for exclusion in ["rate limit", "too many requests"] {
        for text in [
            format!("{exclusion}: too many tokens"),
            format!("too many tokens; {exclusion}"),
            format!("too many tokens; {exclusion}").to_uppercase(),
        ] {
            assert!(!is_context_overflow(&error(&text), None));
        }
    }
    assert!(!is_context_overflow(&error("429 temporary failure"), None));
    assert!(is_context_overflow(&error("429 too many tokens"), None));
}

#[test]
fn bedrock_throttling_tokens_are_not_overflow() {
    assert!(!is_context_overflow(
        &error("Throttling error: Too many tokens, please wait before trying again."),
        Some(200000.0)
    ));
}

#[test]
fn bedrock_unavailable_is_not_overflow() {
    assert!(!is_context_overflow(
        &error("Service unavailable: The service is temporarily unavailable."),
        Some(200000.0)
    ));
}

#[test]
fn rate_limit_text_is_not_overflow() {
    assert!(!is_context_overflow(
        &error("Rate limit exceeded, please retry after 30 seconds."),
        Some(200000.0)
    ));
}

#[test]
fn request_rate_errors_are_not_overflow() {
    assert!(!is_context_overflow(
        &error("Too many requests. Please slow down."),
        Some(200000.0)
    ));
}

#[test]
fn explicit_overflow_needs_error_and_nonempty_text() {
    for text in [None, Some(""), Some("unrelated")] {
        let mut result = error("");
        result.error_message = text.map(str::to_owned);
        assert!(!is_context_overflow(&result, None));
    }
    let mut result = error("prompt is too long");
    for reason in [
        StopReason::Stop,
        StopReason::Length,
        StopReason::ToolUse,
        StopReason::Aborted,
    ] {
        result.stop_reason = reason;
        assert!(!is_context_overflow(&result, None));
    }
    result.stop_reason = StopReason::Error;
    let before = result.clone();
    assert!(is_context_overflow(&result, None));
    assert_eq!(result, before);
}

#[test]
fn silent_overflow_counts_input_plus_cached_input() {
    for (input, cached, expected) in [
        (50.0, 51.0, true),
        (50.0, 50.0, false),
        (50.0, 49.0, false),
        (-10.0, 111.0, true),
        (111.0, -10.0, true),
        (f64::NAN, 111.0, false),
    ] {
        let mut result = message(StopReason::Stop, input, cached, 9999.0);
        result.usage.cache_write = 10000.0;
        result.usage.total_tokens = -1.0;
        result.usage.cost.total = 10000.0;
        assert_eq!(is_context_overflow(&result, Some(100.0)), expected);
    }
    for reason in [
        StopReason::Error,
        StopReason::Length,
        StopReason::ToolUse,
        StopReason::Aborted,
    ] {
        assert!(!is_context_overflow(
            &message(reason, 101.0, 0.0, 1.0),
            Some(100.0)
        ));
    }
}

#[test]
fn xiaomi_full_cached_context_with_no_output_overflows() {
    assert!(is_context_overflow(
        &message(StopReason::Length, 58.0, 1048512.0, 0.0),
        Some(1048576.0)
    ));
}

#[test]
fn length_stop_with_output_is_not_overflow() {
    assert!(!is_context_overflow(
        &message(StopReason::Length, 1000.0, 0.0, 4096.0),
        Some(200000.0)
    ));
}

#[test]
fn length_stop_below_context_is_not_overflow() {
    assert!(!is_context_overflow(
        &message(StopReason::Length, 100.0, 0.0, 0.0),
        Some(200000.0)
    ));
}

#[test]
fn length_overflow_requires_zero_output_at_threshold() {
    for (input, expected) in [(99.0, true), (98.999, false), (100.0, true)] {
        for output in [0.0, -0.0] {
            let mut result = message(StopReason::Length, input - 10.0, 10.0, output);
            result.usage.cache_write = 1e9;
            result.usage.total_tokens = -1.0;
            result.usage.cost.total = 1e9;
            assert_eq!(is_context_overflow(&result, Some(100.0)), expected);
        }
    }
    for output in [1.0, -1.0, f64::NAN] {
        assert!(!is_context_overflow(
            &message(StopReason::Length, 100.0, 0.0, output),
            Some(100.0)
        ));
    }
    for reason in [
        StopReason::Stop,
        StopReason::Error,
        StopReason::ToolUse,
        StopReason::Aborted,
    ] {
        assert!(!is_context_overflow(
            &message(reason, 99.0, 0.0, 0.0),
            Some(100.0)
        ));
    }
}

#[test]
fn overflow_windows_keep_number_truthiness() {
    for window in [None, Some(0.0), Some(-0.0), Some(f64::NAN)] {
        for reason in [StopReason::Stop, StopReason::Length] {
            assert!(!is_context_overflow(
                &message(reason, f64::INFINITY, 0.0, 0.0),
                window
            ));
        }
    }
    for (window, input, reason, expected) in [
        (-1.0, 0.0, StopReason::Stop, true),
        (-1.0, 0.0, StopReason::Length, true),
        (f64::INFINITY, 100.0, StopReason::Stop, false),
        (f64::INFINITY, 100.0, StopReason::Length, false),
        (f64::INFINITY, f64::INFINITY, StopReason::Stop, false),
        (f64::INFINITY, f64::INFINITY, StopReason::Length, true),
        (f64::NEG_INFINITY, 0.0, StopReason::Stop, true),
        (f64::NEG_INFINITY, 0.0, StopReason::Length, true),
    ] {
        assert_eq!(
            is_context_overflow(&message(reason, input, 0.0, 0.0), Some(window)),
            expected
        );
    }
}

#[test]
fn overflow_regex_uses_ecmascript_character_classes() {
    let patterns = get_overflow_patterns();
    let no_body = &patterns[19];
    let whitespace = [
        0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x20, 0xa0, 0x1680, 0x2000, 0x2001, 0x2002, 0x2003, 0x2004,
        0x2005, 0x2006, 0x2007, 0x2008, 0x2009, 0x200a, 0x2028, 0x2029, 0x202f, 0x205f, 0x3000,
        0xfeff,
    ];
    for code in whitespace {
        let c = char::from_u32(code).unwrap();
        for text in [
            format!("413{c}status code (no body)"),
            format!("400 status code{c}(no body)"),
            format!("413{c}(no body)"),
        ] {
            assert!(no_body.test(&text), "{code:x}");
            assert!(is_context_overflow(&error(&text), None));
        }
    }
    for code in [
        0x08, 0x0e, 0x1f, 0x21, 0x85, 0x9f, 0xa1, 0x167f, 0x1681, 0x1fff, 0x200b, 0x2027, 0x202a,
        0x202e, 0x2030, 0x205e, 0x2060, 0x2fff, 0x3001, 0xfefe, 0xff00,
    ] {
        let c = char::from_u32(code).unwrap();
        for text in [
            format!("413{c}status code (no body)"),
            format!("400 status code{c}(no body)"),
        ] {
            assert!(!no_body.test(&text), "{code:x}");
            assert!(!is_context_overflow(&error(&text), None));
        }
    }
    for text in [
        "400(no body)",
        "413(no body)",
        "400 status code(no body)",
        "413status code(no body)",
        "413 (no body) trailing",
    ] {
        assert!(no_body.test(text));
    }
    for text in [
        "429 (no body)",
        " 413 (no body)",
        "\n413 (no body)",
        "413 status  code (no body)",
    ] {
        assert!(!no_body.test(text));
    }
    for digit in ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "123"] {
        for (index, text) in [
            (5, format!("maximum prompt length is {digit}")),
            (7, format!("maximum context length is {digit} tokens")),
            (8, format!("exceeds the limit of {digit}")),
            (
                13,
                format!("too large for model with {digit} maximum context length"),
            ),
        ] {
            assert!(patterns[index].test(&text));
        }
    }
    for digit in ["١", "１"] {
        for (index, text) in [
            (5, format!("maximum prompt length is {digit}")),
            (7, format!("maximum context length is {digit} tokens")),
            (8, format!("exceeds the limit of {digit}")),
            (
                13,
                format!("too large for model with {digit} maximum context length"),
            ),
        ] {
            assert!(!patterns[index].test(&text));
        }
    }
    for separator in ["", " ", "😀", "\u{85}"] {
        assert!(patterns[4].test(&format!("input token count{separator}exceeds the maximum")));
    }
    for separator in ["\n", "\r", "\u{2028}", "\u{2029}"] {
        assert!(!patterns[4].test(&format!("input token count{separator}exceeds the maximum")));
    }
    for text in [
        "prompt too long; exceeded context length",
        "prompt too long; exceeded max context length",
    ] {
        assert!(patterns[15].test(text));
    }
    for text in [
        "prompt too long; exceededmax context length",
        "prompt too long; exceeded max  context length",
        "prompt too long; exceeded  context length",
    ] {
        assert!(!patterns[15].test(text));
    }
    for text in [
        "context_length_exceeded",
        "context length exceeded",
        "context_length exceeded",
        "context length_exceeded",
    ] {
        assert!(patterns[16].test(text));
    }
}

#[test]
fn overflow_regex_records_accepted_casefold_edges() {
    let patterns = get_overflow_patterns();
    for text in [
        "prompt is too long",
        "PROMPT IS TOO LONG",
        "Prompt Is Too Long",
    ] {
        assert!(patterns[0].test(text));
        assert!(is_context_overflow(&error(text), None));
    }
    // The engine folds long-s in this non-Unicode pattern, unlike ECMAScript.
    // Kelvin retains the same false result as ECMAScript without Unicode flags.
    for (index, text, expected) in [
        (0, "prompt iſ too long", true),
        (17, "too many toKens", false),
    ] {
        assert_eq!(patterns[index].test(text), expected);
        assert_eq!(is_context_overflow(&error(text), None), expected);
    }
}

#[test]
fn overflow_documentation_keeps_all_paragraphs() {
    let source = include_str!("../src/options/overflow.rs");
    let docs = source
        .lines()
        .map(|line| {
            line.trim_start()
                .strip_prefix("///")
                .map(|line| line.strip_prefix(' ').unwrap_or(line))
                .unwrap_or("")
        })
        .collect::<Vec<_>>()
        .join("\n");
    let actual = docs
        .split("\n\n")
        .map(|p| p.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect::<Vec<_>>();
    for paragraph in [
        r###"Regex patterns to detect context overflow errors from different providers."###,
        r###"These patterns match error messages returned when the input exceeds
the model's context window."###,
        r###"Provider-specific patterns (with example error messages):"###,
        r###"- Anthropic: "prompt is too long: 213462 tokens > 200000 maximum"
- Anthropic: "413 {\"error\":{\"type\":\"request_too_large\",\"message\":\"Request exceeds the maximum size\"}}"
- OpenAI: "Your input exceeds the context window of this model"
- Google: "The input token count (1196265) exceeds the maximum number of tokens allowed (1048575)"
- xAI: "This model's maximum prompt length is 131072 but the request contains 537812 tokens"
- Groq: "Please reduce the length of the messages or completion"
- OpenRouter: "This endpoint's maximum context length is X tokens. However, you requested about Y tokens"
- llama.cpp: "the request exceeds the available context size, try increasing it"
- LM Studio: "tokens to keep from the initial prompt is greater than the context length"
- GitHub Copilot: "prompt token count of X exceeds the limit of Y"
- MiniMax: "invalid params, context window exceeds limit"
- Kimi For Coding: "Your request exceeded model token limit: X (requested: Y)"
- Cerebras: "400/413 status code (no body)"
- Mistral: "Prompt contains X tokens ... too large for model with Y maximum context length"
- z.ai: Does NOT error, accepts overflow silently - handled via usage.input > contextWindow
- Xiaomi MiMo: Truncates input to fill contextWindow exactly, then returns finish_reason "length"
  with output=0 (no room left to generate). Detected via stopReason "length" + zero output +
  input filling the context window.
- Ollama: Some deployments truncate silently, others return errors like "prompt too long; exceeded max context length by X tokens""###,
        r###"Patterns that indicate non-overflow errors (e.g. rate limiting, server errors).
Error messages matching any of these are excluded from overflow detection
even if they also match an OVERFLOW_PATTERN."###,
        r###"Example: Bedrock formats throttling errors as "ThrottlingException: Too many tokens,
please wait before trying again." which would match the /too many tokens/i overflow
pattern without this exclusion."###,
        r###"Check if an assistant message represents a context overflow error."###,
        r###"This handles two cases:
1. Error-based overflow: Most providers return stopReason "error" with a
   specific error message pattern.
2. Silent overflow: Some providers accept overflow requests and return
   successfully. For these, we check if usage.input exceeds the context window."###,
        r###"## Reliability by Provider"###,
        r###"**Reliable detection (returns error with detectable message):**
- Anthropic: "prompt is too long: X tokens > Y maximum" or "request_too_large"
- OpenAI (Completions & Responses): "exceeds the context window"
- Google Gemini: "input token count exceeds the maximum"
- xAI (Grok): "maximum prompt length is X but request contains Y"
- Groq: "reduce the length of the messages"
- Cerebras: 400/413 status code (no body)
- Mistral: "Prompt contains X tokens ... too large for model with Y maximum context length"
- OpenRouter (all backends): "maximum context length is X tokens"
- llama.cpp: "exceeds the available context size"
- LM Studio: "greater than the context length"
- Kimi For Coding: "exceeded model token limit: X (requested: Y)""###,
        r###"**Unreliable detection:**
- z.ai: Sometimes accepts overflow silently (detectable via usage.input > contextWindow),
  sometimes returns rate limit errors. Pass contextWindow param to detect silent overflow.
- Xiaomi MiMo: Truncates input to fit contextWindow then returns stopReason "length" with
  output=0. Pass contextWindow param to detect via the "filled context + zero output" signal.
- Ollama: May truncate input silently for some setups, but may also return explicit
  overflow errors that match the patterns above. Silent truncation still cannot be
  detected here because we do not know the expected token count."###,
        r###"## Custom Providers"###,
        r###"If you've added custom models via settings.json, this function may not detect
overflow errors from those providers. To add support:"###,
        r###"1. Send a request that exceeds the model's context window
2. Check the errorMessage in the response
3. Create a regex pattern that matches the error
4. The pattern should be added to OVERFLOW_PATTERNS in this file, or
   check the errorMessage yourself before calling this function"###,
        r###"@param message - The assistant message to check
@param context_window - Optional context window size for detecting silent overflow (z.ai)
@returns true if the message indicates a context overflow"###,
        r###"Get the overflow patterns for testing purposes."###,
    ] {
        let expected = paragraph.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(
            actual.contains(&expected),
            "missing documentation: {expected}"
        );
    }
}
