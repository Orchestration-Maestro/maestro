//! Request preparation of the content-generation provider through its public operations.
use crate::chat;
#[cfg(not(target_arch = "wasm32"))]
use crate::content_preparation;
#[cfg(not(target_arch = "wasm32"))]
use crate::environment_rows;

use chat::TestResult;
#[cfg(not(target_arch = "wasm32"))]
use content_preparation::assert_rows;
#[cfg(not(target_arch = "wasm32"))]
use environment_rows::check_by_environment;
use indexmap::IndexMap;
use maestro_models::providers::content::google::{
    GoogleOptions, GoogleThinking, build_params, build_simple_options,
};
use maestro_models::{
    CacheRetention, Cancellation, Context, Fetch, OnPayload, OnResponse, SharedHandle,
    SimpleStreamOptions, StreamOptions, ThinkingLevel, Transport,
};
use serde_json::json;
#[cfg(not(target_arch = "wasm32"))]
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// A one-message context with a system prompt and one tool.
fn context() -> TestResult<Context> {
    Ok(serde_json::from_value(json!({
        "systemPrompt": "system",
        "messages": [{"role": "user", "content": "hello", "timestamp": 1}],
        "tools": [{"name": "inspect", "description": "d", "parameters": {"type": "object"}}]
    }))?)
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn raw_parameters_keep_empty_config_and_model_identity() -> TestResult {
    assert_rows("raw_parameters_keep_empty_config_and_model_identity")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn generation_fields_preserve_supplied_numbers() -> TestResult {
    assert_rows("generation_fields_preserve_supplied_numbers")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn system_instruction_omits_only_empty_text() -> TestResult {
    assert_rows("system_instruction_omits_only_empty_text")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn tool_choice_requires_nonempty_declarations() -> TestResult {
    assert_rows("tool_choice_requires_nonempty_declarations")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn tool_schemas_keep_the_full_schema_field() -> TestResult {
    assert_rows("tool_schemas_keep_the_full_schema_field")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn thinking_requires_an_explicit_request_and_reasoning_support() -> TestResult {
    assert_rows("thinking_requires_an_explicit_request_and_reasoning_support")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn explicit_thinking_levels_override_budget_values() -> TestResult {
    assert_rows("explicit_thinking_levels_override_budget_values")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn explicit_thinking_budgets_are_not_clamped() -> TestResult {
    assert_rows("explicit_thinking_budgets_are_not_clamped")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn disabled_thinking_uses_the_lowest_family_setting() -> TestResult {
    assert_rows("disabled_thinking_uses_the_lowest_family_setting")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn model_family_recognition_uses_unanchored_ascii_patterns() -> TestResult {
    assert_rows("model_family_recognition_uses_unanchored_ascii_patterns")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn simple_efforts_select_each_family_level() -> TestResult {
    assert_rows("simple_efforts_select_each_family_level")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn simple_efforts_select_each_family_budget() -> TestResult {
    assert_rows("simple_efforts_select_each_family_budget")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn custom_budgets_override_only_the_selected_slot() -> TestResult {
    assert_rows("custom_budgets_override_only_the_selected_slot")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn level_families_ignore_custom_budgets() -> TestResult {
    assert_rows("level_families_ignore_custom_budgets")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn simple_absent_reasoning_disables_family_thinking() -> TestResult {
    assert_rows("simple_absent_reasoning_disables_family_thinking")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn simple_token_limits_use_the_base_option_owner() -> TestResult {
    assert_rows("simple_token_limits_use_the_base_option_owner")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn supported_effort_clamping_precedes_family_mapping() -> TestResult {
    assert_rows("supported_effort_clamping_precedes_family_mapping")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn supported_extra_high_uses_the_high_policy() -> TestResult {
    assert_rows("supported_extra_high_uses_the_high_policy")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn nonreasoning_models_never_gain_thinking_config() -> TestResult {
    assert_rows("nonreasoning_models_never_gain_thinking_config")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn simple_key_resolution_preserves_nonempty_values() -> TestResult {
    assert_rows("simple_key_resolution_preserves_nonempty_values")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn simple_missing_key_fails_before_parameter_preparation() -> TestResult {
    assert_rows("simple_missing_key_fails_before_parameter_preparation")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn raw_signal_checks_happen_at_preparation() -> TestResult {
    assert_rows("raw_signal_checks_happen_at_preparation")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn simple_signal_checks_wait_until_parameter_building() -> TestResult {
    assert_rows("simple_signal_checks_wait_until_parameter_building")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn simple_tool_selection_does_not_leak_into_raw_options() -> TestResult {
    assert_rows("simple_tool_selection_does_not_leak_into_raw_options")
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn numeric_payloads_use_the_existing_json_number_semantics() -> TestResult {
    assert_rows("numeric_payloads_use_the_existing_json_number_semantics")
}

#[test]
fn prepared_signals_share_the_callers_cancellation() -> TestResult {
    let model = chat::model(&json!({"api": "google-generative-ai", "provider": "google"}))?;
    let context = context()?;

    let raw_signal = Cancellation::new();
    let raw = GoogleOptions {
        common: StreamOptions {
            signal: Some(raw_signal.clone()),
            ..StreamOptions::default()
        },
        ..GoogleOptions::default()
    };
    let params = build_params(&model, &context, Some(&raw))?;
    let retained = params.config.abort_signal.ok_or("raw signal retained")?;
    assert!(!retained.is_aborted());
    raw_signal.abort();
    assert!(retained.is_aborted());

    let simple_signal = Cancellation::new();
    let simple = SimpleStreamOptions {
        common: StreamOptions {
            api_key: Some("key".into()),
            signal: Some(simple_signal.clone()),
            ..StreamOptions::default()
        },
        ..SimpleStreamOptions::default()
    };
    let selected = build_simple_options(&model, &context, Some(&simple))?;
    let params = build_params(&model, &context, Some(&selected))?;
    let composed = params
        .config
        .abort_signal
        .ok_or("composed signal retained")?;
    let kept = selected.common.signal.ok_or("simple signal retained")?;
    assert!(!composed.is_aborted() && !kept.is_aborted());
    simple_signal.abort();
    assert!(composed.is_aborted() && kept.is_aborted());
    Ok(())
}

/// Hook and transport handles that count their invocations in `calls`.
fn counting_handles(calls: &Arc<AtomicUsize>) -> (OnPayload, OnResponse, Fetch) {
    let payload_calls = Arc::clone(calls);
    let on_payload: OnPayload = SharedHandle::new(move |payload, _| {
        payload_calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(std::future::ready(Ok(payload)))
    });
    let response_calls = Arc::clone(calls);
    let on_response: OnResponse = SharedHandle::new(move |_, _| {
        response_calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(std::future::ready(Ok(())))
    });
    let fetch_calls = Arc::clone(calls);
    let fetch: Fetch = Arc::new(move |_| {
        fetch_calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(std::future::ready(Err(maestro_models::FetchError::Aborted)))
    });
    (on_payload, on_response, fetch)
}

/// Check the plain-data common settings the simple operation must keep.
fn assert_scalar_settings(common: &StreamOptions, metadata: Option<&maestro_models::JsonObject>) {
    assert_eq!(common.temperature, Some(0.25));
    assert_eq!(common.max_tokens, Some(77.0));
    assert_eq!(common.api_key.as_deref(), Some("key"));
    assert_eq!(common.transport, Some(Transport::Sse));
    assert_eq!(common.cache_retention, Some(CacheRetention::Long));
    assert_eq!(common.session_id.as_deref(), Some("session"));
    let names: Vec<&str> = common
        .headers
        .iter()
        .flatten()
        .map(|(k, _)| k.as_str())
        .collect();
    assert_eq!(names, ["x-b", "x-a"]);
    assert_eq!(
        (
            common.timeout_ms,
            common.max_retries,
            common.max_retry_delay_ms
        ),
        (Some(11.0), Some(3.0), Some(13.0))
    );
    assert_eq!(common.metadata.as_ref(), metadata);
}

#[test]
fn simple_options_keep_common_settings_and_hook_handles_without_invocation() -> TestResult {
    let model = chat::model(&json!({"api": "google-generative-ai", "provider": "google"}))?;
    let calls = Arc::new(AtomicUsize::new(0));
    let (on_payload, on_response, fetch) = counting_handles(&calls);
    let mut headers = IndexMap::new();
    headers.insert("x-b".to_owned(), "2".to_owned());
    headers.insert("x-a".to_owned(), "1".to_owned());
    let metadata = json!({"user": "u"}).as_object().cloned();
    let simple = SimpleStreamOptions {
        common: StreamOptions {
            temperature: Some(0.25),
            max_tokens: Some(77.0),
            signal: Some(Cancellation::new()),
            api_key: Some("key".into()),
            transport: Some(Transport::Sse),
            cache_retention: Some(CacheRetention::Long),
            session_id: Some("session".into()),
            on_payload: Some(on_payload.clone()),
            on_response: Some(on_response.clone()),
            headers: Some(headers),
            timeout_ms: Some(11.0),
            max_retries: Some(3.0),
            max_retry_delay_ms: Some(13.0),
            metadata: metadata.clone(),
            fetch: Some(fetch.clone()),
        },
        reasoning: Some(ThinkingLevel::Low),
        ..SimpleStreamOptions::default()
    };
    let context = context()?;
    let selected = build_simple_options(&model, &context, Some(&simple))?;
    let raw = GoogleOptions {
        common: selected.common.clone(),
        ..GoogleOptions::default()
    };
    build_params(&model, &context, Some(&raw))?;
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let common = selected.common;
    assert_scalar_settings(&common, metadata.as_ref());
    assert!(common.signal.is_some());
    assert!(SharedHandle::ptr_eq(
        &common.on_payload.ok_or("payload hook")?,
        &on_payload
    ));
    assert!(SharedHandle::ptr_eq(
        &common.on_response.ok_or("response hook")?,
        &on_response
    ));
    assert!(Arc::ptr_eq(&common.fetch.ok_or("transport")?, &fetch));
    Ok(())
}

#[test]
fn prepared_values_do_not_mutate_the_supplied_context() -> TestResult {
    let model = chat::model(&json!({"api": "google-generative-ai", "provider": "google"}))?;
    let context = context()?;
    let before = serde_json::to_value(&context)?;
    let options = GoogleOptions {
        thinking: Some(GoogleThinking::default()),
        ..GoogleOptions::default()
    };
    let mut params = build_params(&model, &context, Some(&options))?;
    params.contents.clear();
    params.config.system_instruction = Some("changed".into());
    params.config.tools = None;
    assert_eq!(serde_json::to_value(&context)?, before);
    let again = build_params(&model, &context, Some(&options))?;
    assert_eq!(again.contents.len(), 1);
    assert_eq!(again.config.system_instruction.as_deref(), Some("system"));
    assert!(again.config.tools.is_some());
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn absent_options_prepare_like_empty_options() -> TestResult {
    let model = chat::model(&json!({"api": "google-generative-ai", "provider": "google"}))?;
    let environment = BTreeMap::from([("GEMINI_API_KEY".to_owned(), "env-key".to_owned())]);
    check_by_environment(
        "absent_options_prepare_like_empty_options",
        &[environment],
        |environment| Some(environment),
        |_| {
            let context = context()?;
            assert_eq!(
                serde_json::to_value(build_params(&model, &context, None)?)?,
                serde_json::to_value(build_params(
                    &model,
                    &context,
                    Some(&GoogleOptions::default())
                )?)?
            );
            let absent = build_simple_options(&model, &context, None)?;
            let empty =
                build_simple_options(&model, &context, Some(&SimpleStreamOptions::default()))?;
            assert_eq!(absent.common.api_key.as_deref(), Some("env-key"));
            assert_eq!(absent.common.max_tokens, Some(4096.0));
            assert_eq!(
                (absent.common.api_key, absent.common.max_tokens),
                (empty.common.api_key, empty.common.max_tokens)
            );
            let (a, e) = (
                absent.thinking.ok_or("thinking")?,
                empty.thinking.ok_or("thinking")?,
            );
            assert!(!a.enabled && !e.enabled);
            Ok(())
        },
    )
}
