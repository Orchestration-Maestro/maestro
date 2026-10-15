//! Shared assistant state and native launch policy through public surfaces.

use crate::chat;
use maestro_models::providers::assistant_output::initial_message;
use maestro_models::{AssistantMessage, timestamp_now};
use serde_json::Value;

#[test]
fn assistant_initial_state_matches_recorded_fields() -> chat::TestResult {
    let rows: Vec<Value> = serde_json::from_str(include_str!("fixtures/assistant-output.json"))?;
    let mut count = 0;
    for row in rows.iter().filter(|row| row["kind"] == "initial") {
        assert!(
            [
                "chat",
                "messages",
                "responses",
                "sessions",
                "conversation",
                "content",
                "azure",
                "bedrock"
            ]
            .contains(&row["input"]["owner"].as_str().unwrap())
        );
        let model = chat::model(&row["input"]["model"])?;
        let before = timestamp_now();
        let mut actual = initial_message(&model);
        assert!((before..=timestamp_now()).contains(&actual.timestamp));
        actual.timestamp = row["input"]["timestamp"].as_f64().unwrap();
        let expected: AssistantMessage = serde_json::from_value(row["expected"].clone())?;
        if matches!(row["input"]["owner"].as_str(), Some("content" | "bedrock")) {
            // Protocol-specific API identity is covered by the follow-up route correction.
            assert_eq!(actual.api, model.api);
            assert_eq!(
                expected.api,
                if row["input"]["owner"] == "content" {
                    "google-generative-ai"
                } else {
                    "bedrock-converse-stream"
                }
            );
        } else {
            assert_eq!(actual.api, expected.api);
        }
        assert_initial_shared_fields(actual, expected);
        count += 1;
    }
    assert_eq!(count, 16);
    Ok(())
}

/// Compare the complete shared record except protocol-owned API identity.
fn assert_initial_shared_fields(actual: AssistantMessage, expected: AssistantMessage) {
    assert_eq!(
        (
            actual.content,
            actual.provider,
            actual.model,
            actual.response_model,
            actual.response_id,
            actual.diagnostics,
            actual.usage,
            actual.stop_reason,
            actual.error_message,
            actual.timestamp
        ),
        (
            expected.content,
            expected.provider,
            expected.model,
            expected.response_model,
            expected.response_id,
            expected.diagnostics,
            expected.usage,
            expected.stop_reason,
            expected.error_message,
            expected.timestamp
        )
    );
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn producer_startup_preserves_native_failure_policy() -> chat::TestResult {
    use futures_util::FutureExt;
    use maestro_models::{Cancellation, StopReason, StreamOptions};
    for aborted in [false, true] {
        for api in [
            "openai-completions",
            "anthropic-messages",
            "openai-responses",
            "openai-codex-responses",
            "mistral-conversations",
            "google-generative-ai",
            "azure-openai-responses",
            "bedrock-converse-stream",
        ] {
            let signal = Cancellation::new();
            if aborted {
                signal.abort();
            }
            let common = StreamOptions {
                signal: Some(signal),
                api_key: Some("fixture-key".into()),
                ..Default::default()
            };
            let stream = launch_native(api, common)?;
            let output = stream.result().now_or_never().unwrap();
            let message = output.read().unwrap();
            assert_eq!(
                message.error_message.as_deref(),
                Some("Streaming requires a running Tokio runtime.")
            );
            let forwards = !matches!(api, "mistral-conversations" | "google-generative-ai");
            assert_eq!(
                message.stop_reason,
                if aborted && forwards {
                    StopReason::Aborted
                } else {
                    StopReason::Error
                }
            );
        }
    }
    Ok(())
}

/// Start the four shared HTTP protocols with controlled, unpolled request operands.
#[cfg(not(target_arch = "wasm32"))]
fn launch_native(
    api: &str,
    common: maestro_models::StreamOptions,
) -> chat::TestResult<maestro_models::AssistantMessageEventStream> {
    use maestro_models::providers::{
        chat::openai_completions::*, messages::anthropic::*,
        response_sessions::openai_codex_responses::*, responses::openai_responses::*,
    };
    let model = chat::model(
        &serde_json::json!({"api": api, "provider":"fixture", "baseUrl":"https://fixture.invalid"}),
    )?;
    let context = chat::context(&serde_json::json!({"messages":[]}))?;
    Ok(match api {
        "openai-completions" => stream_openai_completions(
            model,
            context,
            Some(OpenAICompletionsOptions {
                common,
                ..Default::default()
            }),
        ),
        "anthropic-messages" => stream_anthropic(
            model,
            context,
            Some(AnthropicOptions {
                common,
                ..Default::default()
            }),
        ),
        "openai-responses" => stream_openai_responses(
            model,
            context,
            Some(OpenAIResponsesOptions {
                common,
                ..Default::default()
            }),
        ),
        "openai-codex-responses" => stream_openai_codex_responses(
            model,
            context,
            Some(OpenAICodexResponsesOptions {
                common,
                ..Default::default()
            }),
        ),
        _ => launch_other(model, context, common),
    })
}

/// Start the remaining protocols without reading credentials or dispatching their requests.
#[cfg(not(target_arch = "wasm32"))]
fn launch_other(
    model: maestro_models::Model,
    context: maestro_models::Context,
    common: maestro_models::StreamOptions,
) -> maestro_models::AssistantMessageEventStream {
    use maestro_models::providers::{
        cloud::amazon_bedrock::*, content::google::*, reasoning::mistral::*,
        responses::azure_openai_responses::*,
    };
    match model.api.as_str() {
        "mistral-conversations" => stream_mistral(
            model,
            context,
            Some(MistralOptions {
                common,
                ..Default::default()
            }),
        ),
        "google-generative-ai" => stream_google(
            model,
            context,
            Some(GoogleOptions {
                common,
                ..Default::default()
            }),
        ),
        "azure-openai-responses" => stream_azure_openai_responses(
            model,
            context,
            Some(AzureOpenAIResponsesOptions {
                common,
                azure_resource_name: Some("fixture".into()),
                ..Default::default()
            }),
        ),
        _ => stream_bedrock(
            model,
            context,
            Some(BedrockOptions {
                common,
                region: Some("us-east-1".into()),
                ..Default::default()
            }),
        ),
    }
}
