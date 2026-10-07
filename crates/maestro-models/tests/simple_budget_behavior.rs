use credential_assertions::assert_credential_eq;
#[path = "support/credential_assertions.rs"]
mod credential_assertions;
use maestro_models::*;

fn model(limit: f64) -> Model {
    Model {
        id: "unregistered".into(),
        name: String::new(),
        api: "arbitrary-unregistered-api".into(),
        provider: "unregistered".into(),
        base_url: String::new(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![],
        cost: TokenRates {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: -1.5,
        max_tokens: limit,
        headers: None,
        compat: None,
    }
}

#[test]
fn base_options_keep_unsupplied_fields_absent() {
    for options in [None, Some(SimpleStreamOptions::default())] {
        let result = build_base_options(&model(0.0), options, None);
        assert_eq!(
            serde_json::to_value(&result).unwrap(),
            serde_json::json!({})
        );
        assert!(result.signal.is_none());
        assert!(result.on_payload.is_none());
        assert!(result.on_response.is_none());
    }
}

#[test]
fn base_options_default_positive_descriptor_limits() {
    for (limit, expected) in [
        (1.0, 1.0),
        (31999.0, 31999.0),
        (32000.0, 32000.0),
        (32001.0, 32000.0),
        (f64::INFINITY, 32000.0),
        (0.25, 0.25),
    ] {
        assert_eq!(
            build_base_options(&model(limit), None, None).max_tokens,
            Some(expected)
        );
    }
    for limit in [0.0, -0.0, -1.0, f64::NEG_INFINITY, f64::NAN] {
        assert!(
            build_base_options(&model(limit), None, None)
                .max_tokens
                .is_none()
        );
    }
}

fn same_number(actual: f64, expected: f64) {
    if expected.is_nan() {
        assert!(actual.is_nan());
    } else {
        assert_eq!(actual.to_bits(), expected.to_bits());
    }
}

#[test]
fn base_options_preserve_explicit_token_values() {
    for value in [
        0.0,
        -0.0,
        -20.0,
        0.25,
        100000.0,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
    ] {
        let options = SimpleStreamOptions {
            base: StreamOptions {
                max_tokens: Some(value),
                ..Default::default()
            },
            ..Default::default()
        };
        same_number(
            build_base_options(&model(10.0), Some(options), None)
                .max_tokens
                .unwrap(),
            value,
        );
    }
}

#[test]
fn base_options_choose_key_without_trimming() {
    for supplied in [None, Some(""), Some("option-key")] {
        for resolved in [
            None,
            Some(""),
            Some("resolved"),
            Some(" "),
            Some("\u{feff}"),
            Some("\u{85}"),
        ] {
            let options = SimpleStreamOptions {
                base: StreamOptions {
                    api_key: supplied.map(str::to_owned),
                    ..Default::default()
                },
                ..Default::default()
            };
            let expected = resolved.filter(|key| !key.is_empty()).or(supplied);
            assert_credential_eq(
                &(build_base_options(&model(0.0), Some(options), resolved)
                    .api_key
                    .as_deref()),
                &(expected),
                "base_options_choose_key_without_trimming",
            );
        }
    }
}

#[test]
fn base_options_forward_every_common_field() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    for transport in [
        Transport::Sse,
        Transport::Websocket,
        Transport::WebsocketCached,
        Transport::Auto,
    ] {
        for retention in [
            CacheRetention::None,
            CacheRetention::Short,
            CacheRetention::Long,
        ] {
            let calls = Arc::new(AtomicUsize::new(0));
            let payload_calls = calls.clone();
            let response_calls = calls.clone();
            let signal = Cancellation::new();
            let headers = serde_json::json!({"z":"last", "a":"first"})
                .as_object()
                .unwrap()
                .clone();
            let metadata = serde_json::json!({"z":0, "a":false})
                .as_object()
                .unwrap()
                .clone();
            let base = StreamOptions {
                temperature: Some(0.0),
                max_tokens: Some(0.0),
                signal: Some(signal.clone()),
                api_key: Some("key".into()),
                transport: Some(transport.clone()),
                cache_retention: Some(retention.clone()),
                session_id: Some(String::new()),
                headers: Some(headers.clone()),
                metadata: Some(metadata.clone()),
                timeout_ms: Some(0.0),
                max_retries: Some(0.0),
                max_retry_delay_ms: Some(0.0),
                on_payload: Some(Arc::new(move |_, _| {
                    payload_calls.fetch_add(1, Ordering::SeqCst);
                    Box::pin(async { Ok(None) })
                })),
                on_response: Some(Arc::new(move |_, _| {
                    response_calls.fetch_add(1, Ordering::SeqCst);
                    Box::pin(async { Ok(()) })
                })),
            };
            let payload = base.on_payload.clone().unwrap();
            let response = base.on_response.clone().unwrap();
            let result = build_base_options(
                &model(999.0),
                Some(SimpleStreamOptions {
                    base,
                    reasoning: Some(ThinkingLevel::Xhigh),
                    thinking_budgets: Some(ThinkingBudgets {
                        minimal: Some(-1.0),
                        low: None,
                        medium: None,
                        high: None,
                    }),
                }),
                None,
            );
            assert_eq!(result.temperature, Some(0.0));
            assert_eq!(result.max_tokens, Some(0.0));
            assert_credential_eq(
                &(result.api_key.as_deref()),
                &(Some("key")),
                "base_options_forward_every_common_field",
            );
            assert_eq!(result.transport, Some(transport.clone()));
            assert_eq!(result.cache_retention, Some(retention));
            assert_eq!(result.session_id.as_deref(), Some(""));
            assert_eq!(result.timeout_ms, Some(0.0));
            assert_eq!(result.max_retries, Some(0.0));
            assert_eq!(result.max_retry_delay_ms, Some(0.0));
            assert_credential_eq(
                &(result.headers),
                &(Some(headers)),
                "base_options_forward_every_common_field",
            );
            assert_credential_eq(
                &(result.metadata),
                &(Some(metadata)),
                "base_options_forward_every_common_field",
            );
            assert_credential_eq(
                &(result.headers.as_ref().unwrap().keys().collect::<Vec<_>>()),
                &(["z", "a"]),
                "base_options_forward_every_common_field",
            );
            assert_credential_eq(
                &(result.metadata.as_ref().unwrap().keys().collect::<Vec<_>>()),
                &(["z", "a"]),
                "base_options_forward_every_common_field",
            );
            assert!(Arc::ptr_eq(&payload, result.on_payload.as_ref().unwrap()));
            assert!(Arc::ptr_eq(&response, result.on_response.as_ref().unwrap()));
            signal.cancel();
            assert!(result.signal.as_ref().unwrap().is_cancelled());
            assert_eq!(calls.load(Ordering::SeqCst), 0);
            let json = serde_json::to_value(&result).unwrap();
            assert!(json.get("reasoning").is_none());
            assert!(json.get("thinkingBudgets").is_none());
        }
    }
}

#[test]
fn reasoning_clamp_preserves_absence_and_other_levels() {
    assert_eq!(clamp_reasoning(None), None);
    for level in [
        ThinkingLevel::Minimal,
        ThinkingLevel::Low,
        ThinkingLevel::Medium,
        ThinkingLevel::High,
    ] {
        assert_eq!(clamp_reasoning(Some(level.clone())), Some(level));
    }
    assert_eq!(
        clamp_reasoning(Some(ThinkingLevel::Xhigh)),
        Some(ThinkingLevel::High)
    );
    assert_eq!(
        serde_json::to_value(ModelThinkingLevel::Off).unwrap(),
        "off"
    );
}

fn budgets() -> ThinkingBudgets {
    ThinkingBudgets {
        minimal: None,
        low: None,
        medium: None,
        high: None,
    }
}

#[test]
fn thinking_budgets_keep_defaults_and_partial_overrides() {
    let levels = [
        ThinkingLevel::Minimal,
        ThinkingLevel::Low,
        ThinkingLevel::Medium,
        ThinkingLevel::High,
        ThinkingLevel::Xhigh,
    ];
    for custom in [None, Some(budgets())] {
        for (level, expected) in levels
            .iter()
            .zip([1024.0, 2048.0, 8192.0, 16384.0, 16384.0])
        {
            let result =
                adjust_max_tokens_for_thinking(100.0, 100000.0, level.clone(), custom.as_ref());
            assert_eq!(result.thinking_budget, expected);
            assert_eq!(result.max_tokens, expected + 100.0);
        }
    }
    for index in 0..4 {
        for value in [0.0, 0.5, -20.0] {
            let mut custom = budgets();
            match index {
                0 => custom.minimal = Some(value),
                1 => custom.low = Some(value),
                2 => custom.medium = Some(value),
                _ => custom.high = Some(value),
            }
            for (i, level) in levels.iter().enumerate() {
                let expected = if i.min(3) == index {
                    value
                } else {
                    [1024.0, 2048.0, 8192.0, 16384.0, 16384.0][i]
                };
                let result =
                    adjust_max_tokens_for_thinking(100.0, 100000.0, level.clone(), Some(&custom));
                assert_eq!(result.thinking_budget, expected);
                assert_eq!(result.max_tokens, expected + 100.0);
            }
        }
    }
    let custom = ThinkingBudgets {
        minimal: Some(1.0),
        low: Some(2.0),
        medium: Some(3.0),
        high: Some(4.0),
    };
    for (level, expected) in levels.into_iter().zip([1.0, 2.0, 3.0, 4.0, 4.0]) {
        assert_eq!(
            adjust_max_tokens_for_thinking(100.0, 100000.0, level, Some(&custom)).thinking_budget,
            expected
        );
    }
}

#[test]
fn thinking_budget_reduction_uses_the_exact_guard() {
    for (base, model, level, max, thinking) in [
        (4096.0, 8192.0, ThinkingLevel::High, 8192.0, 7168.0),
        (0.0, 512.0, ThinkingLevel::Minimal, 512.0, 0.0),
        (0.0, 1024.0, ThinkingLevel::Minimal, 1024.0, 0.0),
        (0.0, 2048.0, ThinkingLevel::High, 2048.0, 1024.0),
        (1.0, 100000.0, ThinkingLevel::High, 16385.0, 16384.0),
        (1.0, 1024.5, ThinkingLevel::Minimal, 1024.5, 1024.0),
        (1.0, 1024.0, ThinkingLevel::Minimal, 1024.0, 0.0),
        (1.0, 1023.5, ThinkingLevel::Minimal, 1023.5, 0.0),
        (1.0, 1026.0, ThinkingLevel::Minimal, 1025.0, 1024.0),
    ] {
        let result = adjust_max_tokens_for_thinking(base, model, level, None);
        same_number(result.max_tokens, max);
        same_number(result.thinking_budget, thinking);
    }
}

#[test]
fn thinking_arithmetic_preserves_binary64_edges() {
    for (base, model, budget, max, thinking) in [
        (f64::NAN, 10000.0, 1024.0, f64::NAN, 1024.0),
        (100.0, f64::NAN, 1024.0, f64::NAN, 1024.0),
        (100.0, 10000.0, f64::NAN, f64::NAN, f64::NAN),
        (f64::INFINITY, f64::INFINITY, 1024.0, f64::INFINITY, 1024.0),
        (
            f64::INFINITY,
            10000.0,
            f64::NEG_INFINITY,
            f64::NAN,
            f64::NEG_INFINITY,
        ),
        (100.0, -0.0, 1024.0, -0.0, 0.0),
        (-0.0, 0.0, -0.0, -0.0, 0.0),
        (100.0, -50.0, 1024.0, -50.0, 0.0),
        (100.0, 50.0, -20.0, 50.0, -20.0),
        (100.0, f64::NEG_INFINITY, 1024.0, f64::NEG_INFINITY, 0.0),
        (
            100.0,
            f64::INFINITY,
            f64::INFINITY,
            f64::INFINITY,
            f64::INFINITY,
        ),
        (
            9007199254740992.0,
            f64::INFINITY,
            1.0,
            9007199254740992.0,
            1.0,
        ),
        (
            9007199254740991.0,
            f64::INFINITY,
            1.0,
            9007199254740992.0,
            1.0,
        ),
        (1024.0, 0.0, -1024.0, 0.0, -1024.0),
    ] {
        let custom = ThinkingBudgets {
            minimal: Some(budget),
            ..budgets()
        };
        let result =
            adjust_max_tokens_for_thinking(base, model, ThinkingLevel::Minimal, Some(&custom));
        same_number(result.max_tokens, max);
        same_number(result.thinking_budget, thinking);
    }
}

#[test]
fn model_options_guide_keeps_each_contract_paragraph() {
    let guide = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/model-options.md"
    ))
    .expect("model options guide exists");
    for expected in [
        r###"`stream` forwards `ProviderStreamOptions` to the registered raw callback; `stream_simple` forwards `SimpleStreamOptions` to its separate simple callback. Absent options remain absent. `StreamOptions` holds the common fields, including api key, signal, temperature, max tokens, transport, cache retention, session ID, headers, callback hooks, retry fields and metadata. Raw options also retain provider-specific extras; simple options retain reasoning and thinking budgets."###,
        r###"Invocation dispatch does not choose defaults, clamp limits, resolve authentication, merge headers or infer provider capabilities. The descriptor is supplied data. Signals are forwarded unchanged; cancelling a signal does not itself close a producer-owned stream. Simple provider adapters call `build_base_options` when they need common simple defaults; the raw path does not call this helper automatically."###,
        r###"`build_base_options` forwards all common fields. An omitted max-token value defaults to the smaller of a positive descriptor limit and 32000; a nonpositive limit leaves it absent. Explicit numeric values are preserved. A nonempty resolved key wins over the supplied option key; keys are not trimmed. `clamp_reasoning` maps Xhigh to High and preserves the other levels or absence."###,
        r###"`adjust_max_tokens_for_thinking` uses minimal 1024, low 2048, medium 8192 and high 16384 unless individually overridden; Xhigh uses High. It caps base plus thinking at the supplied model limit. Only when that total is no greater than thinking does it reduce thinking to the greater of zero and total minus 1024. It does not guarantee 1024 output tokens otherwise, validate limits or round numbers to integers."###,
        r###"`is_context_overflow` checks nonempty error text against exclusions before positive patterns. Successful Stop results overflow when input plus cached input exceeds a supplied truthy context window. Length results with zero output overflow when that sum reaches 99% of the window. Other outcomes remain distinct. `get_overflow_patterns` returns a copied ordered pattern collection. The helpers do not invoke providers, read credentials, mutate messages or print diagnostics."###,
    ] {
        assert!(
            guide.split("\n\n").any(|p| p == expected),
            "missing guide paragraph: {expected}"
        );
    }
    assert!(guide.contains(
        r###"```rust
use maestro_models::{ThinkingLevel, adjust_max_tokens_for_thinking, clamp_reasoning};

assert_eq!(clamp_reasoning(Some(ThinkingLevel::Xhigh)), Some(ThinkingLevel::High));
let adjusted = adjust_max_tokens_for_thinking(4096.0, 8192.0, ThinkingLevel::High, None);
assert_eq!(adjusted.max_tokens, 8192.0);
assert_eq!(adjusted.thinking_budget, 7168.0);
```"###
    ));
}
