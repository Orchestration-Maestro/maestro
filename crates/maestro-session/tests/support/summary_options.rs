//! Shared consuming request control assertions.
use maestro_models::SimpleStreamOptions;
use serde_json::Value;
/// Compare controls without displaying authentication or header values.
pub fn compare_options(options: &SimpleStreamOptions, expected: &Value) {
    assert_eq!(options.common.max_tokens, expected["maxTokens"].as_f64());
    let no_unrelated_options = options.common.temperature.is_none()
        && options.common.transport.is_none()
        && options.common.cache_retention.is_none()
        && options.common.session_id.is_none()
        && options.common.on_payload.is_none()
        && options.common.on_response.is_none()
        && options.common.timeout_ms.is_none()
        && options.common.max_retries.is_none()
        && options.common.max_retry_delay_ms.is_none()
        && options.common.metadata.is_none()
        && options.common.fetch.is_none()
        && options.thinking_budgets.is_none()
        && options.tool_choice.is_none();
    assert!(no_unrelated_options);

    // Never display authentication or header values in failure messages.
    let same_key = options.common.api_key.as_deref() == expected["apiKey"].as_str();
    assert!(same_key, "authentication differs");
    let same_headers =
        serde_json::to_value(&options.common.headers).unwrap() == expected["headers"];
    assert!(same_headers, "headers differ");
    if let Some(headers) = &options.common.headers {
        assert_eq!(
            headers.keys().map(String::as_str).collect::<Vec<_>>(),
            expected["headers"]
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>()
        );
    }
    assert_eq!(
        serde_json::to_value(options.reasoning).unwrap(),
        expected["reasoning"]
    );
    assert_eq!(
        options
            .common
            .signal
            .as_ref()
            .map(maestro_models::Cancellation::is_aborted),
        expected
            .get("signal")
            .map(|v| v["aborted"].as_bool().unwrap())
    );
}

/// Record the controls used by summary consumers, rejecting unexpected request hooks.
pub fn observe<E: maestro_models::ProviderExecution>(
    options: SimpleStreamOptions<E>,
) -> SimpleStreamOptions {
    let maestro_models::StreamOptions {
        base_url,
        temperature,
        max_tokens,
        signal,
        api_key,
        transport,
        cache_retention,
        session_id,
        on_payload,
        on_response,
        headers,
        timeout_ms,
        max_retries,
        max_retry_delay_ms,
        metadata,
        fetch,
    } = options.common;
    assert!(on_payload.is_none());
    assert!(on_response.is_none());
    SimpleStreamOptions {
        common: maestro_models::StreamOptions {
            base_url,
            temperature,
            max_tokens,
            signal,
            api_key,
            transport,
            cache_retention,
            session_id,
            on_payload: None,
            on_response: None,
            headers,
            timeout_ms,
            max_retries,
            max_retry_delay_ms,
            metadata,
            fetch,
        },
        reasoning: options.reasoning,
        thinking_budgets: options.thinking_budgets,
        tool_choice: options.tool_choice,
    }
}
