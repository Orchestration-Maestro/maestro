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
            // Generic initialization keeps the supplied API; protocol streams choose their own.
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

#[test]
fn content_stream_keeps_its_protocol_identity() -> chat::TestResult {
    crate::runtime::run(protocol_identity("content", "google-generative-ai"))
}

#[test]
fn cloud_content_stream_keeps_its_protocol_identity() -> chat::TestResult {
    crate::runtime::run(protocol_identity("content", "google-vertex"))
}

#[test]
fn cloud_response_stream_keeps_its_protocol_identity() -> chat::TestResult {
    crate::runtime::run(protocol_identity("content", "azure-openai-responses"))
}

#[test]
fn cloud_conversation_keeps_its_protocol_identity() -> chat::TestResult {
    crate::runtime::run(protocol_identity("bedrock", "bedrock-converse-stream"))
}

/// Replay both recorded identities and a matching-API control through each outcome.
async fn protocol_identity(owner: &str, api: &str) -> chat::TestResult {
    let rows: Vec<Value> = serde_json::from_str(include_str!("fixtures/assistant-output.json"))?;
    let rows: Vec<_> = rows
        .into_iter()
        .filter(|row| row["kind"] == "initial" && row["input"]["owner"] == owner)
        .collect();
    assert_eq!(rows.len(), 2);
    let mut control = rows[0].clone();
    control["input"]["model"]["api"] = api.into();
    for mut row in rows.into_iter().chain([control]) {
        row["expected"]["api"] = api.into();
        for fail in [false, true] {
            identity_invocation(&row, api, fail).await?;
        }
    }
    Ok(())
}

/// Observe the initialized record before draining and compare all terminal observations.
async fn identity_invocation(row: &Value, api: &str, fail: bool) -> chat::TestResult {
    use maestro_models::{AssistantMessageEvent as Event, DoneReason, ErrorReason, StopReason};
    use std::sync::Arc;
    let model = chat::model(&row["input"]["model"])?;
    let expected: AssistantMessage = serde_json::from_value(row["expected"].clone())?;
    let (common, scripted, calls) = identity_options(&model, api, fail)?;
    let before = timestamp_now();
    let stream = identity_stream(model, api, common)?;
    let first = stream.next().await.ok_or("missing first event")?;
    let handle = match (fail, &first) {
        (false, Event::Start { partial }) => partial,
        (
            true,
            Event::Error {
                error,
                reason: ErrorReason::Error,
            },
        ) => error,
        _ => return Err("unexpected first identity event".into()),
    };
    let mut expected = expected;
    if fail {
        expected.stop_reason = StopReason::Error;
        expected.error_message = Some("identity fixture failure".into());
    }
    assert_identity_record(handle, &expected, before)?;
    if !fail {
        let done = stream.next().await.ok_or("missing done event")?;
        let Event::Done {
            reason: DoneReason::Stop,
            message,
        } = done
        else {
            return Err("unexpected terminal identity event".into());
        };
        assert!(Arc::ptr_eq(handle, &message));
    }
    assert!(stream.next().await.is_none());
    let result = stream.result().await;
    assert!(Arc::ptr_eq(handle, &result));
    assert_identity_record(&result, &expected, before)?;
    assert_eq!(scripted.seen()?.len(), usize::from(!fail));
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    Ok(())
}

/// Compare every field on a test-owned snapshot, with a bounded real timestamp.
fn assert_identity_record(
    handle: &maestro_models::SharedAssistantMessage,
    expected: &AssistantMessage,
    before: f64,
) -> chat::TestResult {
    let mut actual = handle.read().map_err(|e| e.to_string())?.clone();
    assert!((before..=timestamp_now()).contains(&actual.timestamp));
    actual.timestamp = expected.timestamp;
    assert_eq!(&actual, expected);
    Ok(())
}

/// Install a controlled response and a hook that observes the original descriptor.
fn identity_options(
    model: &maestro_models::Model,
    api: &str,
    fail: bool,
) -> chat::TestResult<(
    maestro_models::StreamOptions,
    crate::transport::Transport,
    std::sync::Arc<std::sync::atomic::AtomicUsize>,
)> {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    let scripted = if api == "bedrock-converse-stream" {
        crate::bedrock::serve(vec![crate::bedrock::event(
            "messageStart",
            &serde_json::json!({"role":"assistant"}),
        )?])
    } else {
        crate::transport::transport(vec![crate::transport::Attempt::body(
            200,
            &[("content-type", "text/event-stream")],
            if api == "azure-openai-responses" {
                b"data: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\"}}\n\n".to_vec()
            } else {
                vec![]
            },
        )])
    };
    let supplied = (model.api.clone(), model.provider.clone(), model.id.clone());
    let calls = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&calls);
    let common: maestro_models::StreamOptions = maestro_models::StreamOptions {
        api_key: Some("fixture-key".into()),
        fetch: Some(scripted.fetch.clone()),
        on_payload: Some(Arc::new(
            move |input, model: std::sync::Arc<maestro_models::Model>| {
                assert_eq!(
                    (&model.api, &model.provider, &model.id),
                    (&supplied.0, &supplied.1, &supplied.2)
                );
                count.fetch_add(1, Ordering::SeqCst);
                let result = if fail {
                    Err(crate::bedrock::diagnostic("identity fixture failure"))
                } else {
                    Ok(input)
                };
                Box::pin(std::future::ready(result))
            },
        )),
        ..Default::default()
    };
    Ok((common, scripted, calls))
}

/// Invoke the public direct entry, never selecting a provider by the supplied API.
fn identity_stream(
    model: maestro_models::Model,
    api: &str,
    common: maestro_models::StreamOptions,
) -> chat::TestResult<maestro_models::AssistantMessageEventStream> {
    use maestro_models::providers::{
        cloud::amazon_bedrock::*,
        content::{google::*, google_vertex::*},
        responses::azure_openai_responses::*,
    };
    let context = chat::context(
        &serde_json::json!({"messages": [{"role":"user", "content":"question", "timestamp":1}]}),
    )?;
    Ok(match api {
        "google-generative-ai" => stream_google(
            model,
            context,
            Some(GoogleOptions {
                common,
                ..Default::default()
            }),
        ),
        "google-vertex" => stream_google_vertex(
            model,
            context,
            Some(GoogleVertexOptions {
                common,
                ..Default::default()
            }),
        ),
        "bedrock-converse-stream" => stream_bedrock(
            model,
            context,
            Some(BedrockOptions {
                common,
                bearer_token: Some("fixture-token".into()),
                region: Some("us-east-1".into()),
                ..Default::default()
            }),
        ),
        "azure-openai-responses" => stream_azure_openai_responses(
            model,
            context,
            Some(AzureOpenAIResponsesOptions {
                common,
                azure_base_url: Some("https://fixture.invalid/v1".into()),
                azure_deployment_name: Some("fixture-deployment".into()),
                ..Default::default()
            }),
        ),
        _ => return Err("unknown identity protocol".into()),
    })
}
