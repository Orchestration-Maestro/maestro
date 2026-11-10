//! External response-session records and operation paths.

#[cfg(not(target_arch = "wasm32"))]
use maestro_models::providers::response_sessions::codex::*;
#[cfg(not(target_arch = "wasm32"))]
use maestro_models::{Context, Model, OpenAICodexResponsesOptions, OpenAICodexWebSocketDebugStats};

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn maestro_response_sessions_public_surface() {
    let options = public_records();
    let model: Model = serde_json::from_value(serde_json::json!({"id":"fixture","name":"fixture","api":"openai-codex-responses","provider":"fixture-no-key","baseUrl":"http://localhost","reasoning":false,"input":["text"],"cost":{"input":1,"output":2,"cacheRead":0,"cacheWrite":0},"contextWindow":1000,"maxTokens":100})).unwrap();
    let context = Context {
        system_prompt: None,
        messages: vec![],
        tools: None,
    };
    let error = stream_simple_openai_codex_responses(model.clone(), context.clone(), None)
        .err()
        .unwrap();
    assert_eq!(error.message, "No API key for provider: fixture-no-key");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let stream = stream_openai_codex_responses(model, context, Some(options));
        assert_eq!(
            stream
                .result()
                .await
                .read()
                .unwrap()
                .error_message
                .as_deref(),
            Some("No API key for provider: fixture-no-key")
        );
    });
    reset_openai_codex_web_socket_debug_stats(Some("public-surface"));
    close_openai_codex_web_socket_sessions(Some("public-surface"));
    assert!(get_openai_codex_web_socket_debug_stats("public-surface").is_none());
}

#[cfg(not(target_arch = "wasm32"))]
/// Construct every published field and nullable/union representation externally.
#[cfg(test)]
fn public_records() -> OpenAICodexResponsesOptions {
    let options = OpenAICodexResponsesOptions {
        common: maestro_models::StreamOptions::default(),
        service_tier: Some(Nullable::Null),
        reasoning_effort: Some(maestro_models::ModelThinkingLevel::Off),
        reasoning_summary: Some(OpenAICodexReasoningSummary::Detailed),
        text_verbosity: Some(OpenAICodexTextVerbosity::High),
    };
    let stats = OpenAICodexWebSocketDebugStats {
        requests: 1,
        connections_created: 2,
        connections_reused: 3,
        cached_context_requests: 4,
        store_true_requests: 5,
        full_context_requests: 6,
        delta_requests: 7,
        last_input_items: 8,
        last_delta_input_items: Some(9),
        last_previous_response_id: Some("previous".into()),
        websocket_failures: 10,
        sse_fallbacks: 11,
        websocket_fallback_active: Some(true),
        last_web_socket_error: Some("closed".into()),
    };
    assert_eq!(stats.last_delta_input_items, Some(9));
    assert!(matches!(options.service_tier, Some(Nullable::Null)));
    for summary in [
        OpenAICodexReasoningSummary::Auto,
        OpenAICodexReasoningSummary::Concise,
        OpenAICodexReasoningSummary::Detailed,
        OpenAICodexReasoningSummary::Off,
        OpenAICodexReasoningSummary::On,
    ] {
        assert!(serde_json::to_value(summary).unwrap().is_string());
    }
    for verbosity in [
        OpenAICodexTextVerbosity::Low,
        OpenAICodexTextVerbosity::Medium,
        OpenAICodexTextVerbosity::High,
    ] {
        assert!(serde_json::to_value(verbosity).unwrap().is_string());
    }
    let value = Nullable::Value(
        maestro_models::providers::responses_api::openai::OpenAIResponsesServiceTier::Priority,
    );
    assert_eq!(serde_json::to_value(value).unwrap(), "priority");
    options
}
