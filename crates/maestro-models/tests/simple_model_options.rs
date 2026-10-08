#[cfg(test)]
mod tests {
    use maestro_models::{
        CacheRetention, Cancellation, Model, OnPayload, OnResponse, ProviderResponse,
        SimpleStreamOptions, StreamOptions, Transport, build_base_options,
    };
    use std::{
        collections::BTreeMap,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        task::{Context, Poll, Waker},
    };

    fn model(max_tokens: f64) -> Model {
        serde_json::from_value(serde_json::json!({
            "id": "controlled", "name": "Controlled", "api": "fixture", "provider": "fixture",
            "baseUrl": "https://fixture.invalid", "reasoning": true, "input": ["text"],
            "contextWindow": 100_000, "maxTokens": max_tokens,
            "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0}
        }))
        .unwrap()
    }

    #[test]
    fn maestro_simple_options_preserve_explicit_values() {
        let calls = Arc::new(AtomicUsize::new(0));
        let payload_calls = Arc::clone(&calls);
        let on_payload: OnPayload = Arc::new(move |value, _| {
            payload_calls.fetch_add(1, Ordering::SeqCst);
            Box::pin(async move { Ok(Some(value)) })
        });
        let response_calls = Arc::clone(&calls);
        let on_response: OnResponse = Arc::new(move |_, _| {
            response_calls.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Ok(()) })
        });
        let signal = Cancellation::new();
        let mut options = SimpleStreamOptions {
            common: StreamOptions {
                temperature: Some(0.25),
                max_tokens: Some(-7.0),
                signal: Some(signal.clone()),
                api_key: Some("supplied".into()),
                transport: Some(Transport::WebsocketCached),
                cache_retention: Some(CacheRetention::Long),
                session_id: Some("session".into()),
                headers: Some(BTreeMap::from([("header".into(), "value".into())])),
                on_payload: Some(Arc::clone(&on_payload)),
                on_response: Some(Arc::clone(&on_response)),
                timeout_ms: Some(17.0),
                max_retries: Some(3.0),
                max_retry_delay_ms: Some(29.0),
                metadata: Some(serde_json::from_value(serde_json::json!({"marker": 42})).unwrap()),
            },
            ..Default::default()
        };
        let base = build_base_options(&model(8.0), Some(&options), None);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_common_fields(&base, &options.common);
        assert!(Arc::ptr_eq(base.on_payload.as_ref().unwrap(), &on_payload));
        assert!(Arc::ptr_eq(
            base.on_response.as_ref().unwrap(),
            &on_response
        ));
        signal.abort();
        assert!(base.signal.as_ref().unwrap().is_aborted());
        let mut cx = Context::from_waker(Waker::noop());
        let mut payload = base.on_payload.unwrap()(serde_json::json!("kept"), model(8.0));
        assert!(
            matches!(payload.as_mut().poll(&mut cx), Poll::Ready(Ok(Some(value))) if value == "kept")
        );
        let mut response = base.on_response.unwrap()(ProviderResponse::default(), model(8.0));
        assert!(matches!(
            response.as_mut().poll(&mut cx),
            Poll::Ready(Ok(()))
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        assert_eq!(options.common.max_tokens, Some(-7.0));
        options.common.max_tokens = Some(0.0);
        assert_eq!(
            build_base_options(&model(8.0), Some(&options), None).max_tokens,
            Some(0.0)
        );
    }

    #[test]
    fn maestro_base_options_apply_only_missing_token_and_key_defaults() {
        for (limit, expected) in [
            (12.0, Some(12.0)),
            (32000.0, Some(32000.0)),
            (64000.0, Some(32000.0)),
            (0.0, None),
            (-1.0, None),
        ] {
            let base = build_base_options(&model(limit), None, None);
            assert_eq!(base.max_tokens, expected);
            assert!(base.temperature.is_none() && base.api_key.is_none() && base.signal.is_none());
            assert!(
                base.transport.is_none()
                    && base.cache_retention.is_none()
                    && base.session_id.is_none()
            );
            assert!(base.headers.is_none() && base.metadata.is_none());
            assert!(base.on_payload.is_none() && base.on_response.is_none());
            assert!(
                base.timeout_ms.is_none()
                    && base.max_retries.is_none()
                    && base.max_retry_delay_ms.is_none()
            );
            assert_eq!(
                build_base_options(&model(limit), Some(&SimpleStreamOptions::default()), None)
                    .max_tokens,
                expected
            );
        }
        let mut options = SimpleStreamOptions::default();
        options.common.max_tokens = Some(90000.0);
        options.common.api_key = Some("option-key".into());
        for (key, expected) in [
            (None, "option-key"),
            (Some(""), "option-key"),
            (Some("separate"), "separate"),
            (Some("  "), "  "),
        ] {
            let base = build_base_options(&model(10.0), Some(&options), key);
            assert_eq!(base.max_tokens, Some(90000.0));
            assert_eq!(base.api_key.as_deref(), Some(expected));
        }
        options.common.api_key = Some(String::new());
        assert_eq!(
            build_base_options(&model(10.0), Some(&options), Some(""))
                .api_key
                .as_deref(),
            Some("")
        );
        assert_eq!(
            build_base_options(&model(10.0), None, Some("key"))
                .api_key
                .as_deref(),
            Some("key")
        );
    }

    #[test]
    fn maestro_reasoning_clamp_preserves_supported_levels() {
        use maestro_models::{ThinkingLevel, clamp_reasoning};
        assert_eq!(clamp_reasoning(None), None);
        for level in [
            ThinkingLevel::Minimal,
            ThinkingLevel::Low,
            ThinkingLevel::Medium,
            ThinkingLevel::High,
        ] {
            assert_eq!(clamp_reasoning(Some(level)), Some(level));
        }
        assert_eq!(
            clamp_reasoning(Some(ThinkingLevel::Xhigh)),
            Some(ThinkingLevel::High)
        );
    }

    #[test]
    fn maestro_thinking_budget_reduces_only_when_constrained() {
        use maestro_models::{ThinkingLevel, adjust_max_tokens_for_thinking};
        for (level, total, budget) in [
            (ThinkingLevel::Minimal, 2024.0, 1024.0),
            (ThinkingLevel::Low, 3048.0, 2048.0),
            (ThinkingLevel::Medium, 9192.0, 8192.0),
            (ThinkingLevel::High, 17384.0, 16384.0),
            (ThinkingLevel::Xhigh, 17384.0, 16384.0),
        ] {
            let adjusted = adjust_max_tokens_for_thinking(1000.0, 100_000.0, level, None);
            assert_eq!(
                (adjusted.max_tokens, adjusted.thinking_budget),
                (total, budget)
            );
        }
        for (base, limit, expected) in [
            (16.0, 10000.0, (2064.0, 2048.0)),
            (4096.0, 3000.0, (3000.0, 2048.0)),
            (0.0, 2048.0, (2048.0, 1024.0)),
            (4096.0, 1000.0, (1000.0, 0.0)),
            (100.0, 0.0, (0.0, 0.0)),
            (-100.0, 10000.0, (1948.0, 924.0)),
        ] {
            let adjusted = adjust_max_tokens_for_thinking(base, limit, ThinkingLevel::Low, None);
            assert_eq!((adjusted.max_tokens, adjusted.thinking_budget), expected);
        }
        assert_custom_budgets();
    }

    fn assert_custom_budgets() {
        use maestro_models::{ThinkingBudgets, ThinkingLevel, adjust_max_tokens_for_thinking};
        let custom = ThinkingBudgets {
            minimal: Some(0.0),
            low: Some(7.0),
            medium: None,
            high: Some(99.0),
        };
        let original = custom.clone();
        for (level, expected) in [
            (ThinkingLevel::Minimal, 0.0),
            (ThinkingLevel::Low, 7.0),
            (ThinkingLevel::Medium, 8192.0),
            (ThinkingLevel::High, 99.0),
            (ThinkingLevel::Xhigh, 99.0),
        ] {
            let adjusted = adjust_max_tokens_for_thinking(1000.0, 100_000.0, level, Some(&custom));
            assert_eq!(Some(adjusted.thinking_budget), Some(expected));
        }
        let medium_override = ThinkingBudgets {
            medium: Some(31.0),
            ..Default::default()
        };
        let adjusted = adjust_max_tokens_for_thinking(
            1000.0,
            100_000.0,
            ThinkingLevel::Medium,
            Some(&medium_override),
        );
        assert_eq!(
            (adjusted.max_tokens, adjusted.thinking_budget),
            (1031.0, 31.0)
        );
        assert_eq!(custom, original);
        let absent = ThinkingBudgets::default();
        for (level, budget) in [
            (ThinkingLevel::Minimal, 1024.0),
            (ThinkingLevel::Low, 2048.0),
            (ThinkingLevel::High, 16384.0),
        ] {
            assert_eq!(
                Some(
                    adjust_max_tokens_for_thinking(1000.0, 100_000.0, level, Some(&absent))
                        .thinking_budget
                ),
                Some(budget)
            );
        }
    }

    fn assert_common_fields(base: &StreamOptions, original: &StreamOptions) {
        assert_eq!(base.temperature, Some(0.25));
        assert_eq!(base.max_tokens, Some(-7.0));
        assert_eq!(base.api_key.as_deref(), Some("supplied"));
        assert_eq!(base.transport, Some(Transport::WebsocketCached));
        assert_eq!(base.cache_retention, Some(CacheRetention::Long));
        assert_eq!(base.session_id.as_deref(), Some("session"));
        assert_eq!(base.headers, original.headers);
        assert_eq!(base.metadata, original.metadata);
        assert_eq!(
            (base.timeout_ms, base.max_retries, base.max_retry_delay_ms),
            (Some(17.0), Some(3.0), Some(29.0))
        );
    }
}
