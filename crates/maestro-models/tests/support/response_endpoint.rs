//! Controlled public response invocations with finite producers and consumed observations.
#[cfg(not(target_arch = "wasm32"))]
use crate::chat::TestResult;
#[cfg(not(target_arch = "wasm32"))]
use crate::environment_rows::selected;
#[cfg(not(target_arch = "wasm32"))]
use crate::response_support::{
    Log, body_keys, common as common_options, compare, failure, fetch as shared_fetch, history,
    observe, option, reasoning_summary, request, target as target_with_descriptor,
};
#[cfg(not(target_arch = "wasm32"))]
use maestro_models::providers::responses_api::openai::{
    OpenAIResponsesServiceTier, stream_openai_responses, stream_simple_openai_responses,
};
#[cfg(not(target_arch = "wasm32"))]
use maestro_models::{
    AssistantMessageEventStream, Cancellation, Fetch, OnPayload, OnResponse,
    OpenAIResponsesOptions, StreamOptions,
};
#[cfg(not(target_arch = "wasm32"))]
use serde_json::{Value, json};
#[cfg(not(target_arch = "wasm32"))]
use std::sync::{Arc, PoisonError};

/// Reduced recorded cases.
pub const FIXTURE: &str = include_str!("../fixtures/response_endpoint_cases.json");
#[cfg(not(target_arch = "wasm32"))]
/// Finish event used by request-only cases.
pub const COMPLETE: &str = "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"r-final\",\"status\":\"completed\",\"usage\":{\"input_tokens\":1000000,\"output_tokens\":2000000,\"total_tokens\":3000000,\"input_tokens_details\":{\"cached_tokens\":250000}}}}\n\n";
#[cfg(not(target_arch = "wasm32"))]
/// Model with the recording's rates and limits.
fn target(case: &Value) -> TestResult<maestro_models::Model> {
    target_with_descriptor(
        case,
        json!({"id":"gpt-5.4","name":"Controlled","api":"openai-responses","provider":"openai","baseUrl":"https://controlled.invalid/v1","cost":{"input":2,"output":7,"cacheRead":0.3,"cacheWrite":0.9},"contextWindow":100_000,"maxTokens":64_000}),
    )
}
#[cfg(not(target_arch = "wasm32"))]
/// Controlled transport with provider-specific wire observations.
fn fetch(case: &Value, log: &Log, signal: Cancellation) -> TestResult<Fetch> {
    let ordered = case["hook"] == "order";
    let scalar = case["hook"] == "scalar";
    shared_fetch(case, log, signal, COMPLETE, move |request_value| {
        let mut captured = request(request_value);
        if ordered {
            body_keys(&mut captured);
            captured["wire"] = json!(std::str::from_utf8(&request_value.body).ok());
        }
        if scalar {
            captured["wire"] = json!(request_value.body);
        }
        captured
    })
}
#[cfg(not(target_arch = "wasm32"))]
/// Payload edits reach the transport, and failures prevent it.
fn payload_hook(case: &Value, log: &Log) -> OnPayload {
    let (recorded, hook) = (
        Arc::clone(&log.hooks),
        case["hook"].as_str().unwrap_or_default().to_owned(),
    );
    let replacement = case["payload"].clone();
    Arc::new(
        move |maestro_models::PayloadHookInput {
                  mut payload,
                  signal: hook_signal,
              },
              _| {
            let label = if hook == "order" {
                json!(
                    payload
                        .as_object()
                        .map(|payload| payload.keys().collect::<Vec<_>>())
                )
            } else {
                json!("payload")
            };
            recorded
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(label);
            let result = match hook.as_str() {
                "payload_failure" => Err(failure("payload failed")),
                "null" => Ok(Value::Null),
                "array" => Ok(json!(["replacement", {"retained":true} ])),
                "scalar" => Ok(replacement.clone()),
                "nonstream" => {
                    payload["stream"] = json!(false);
                    Ok(payload)
                }
                "replace" => Ok(json!({"stream":true,"marker":"replacement"})),
                "mutate" => {
                    payload["marker"] = json!("mutated");
                    Ok(payload)
                }
                _ => Ok(payload),
            };
            Box::pin(std::future::ready(result.map(|payload| {
                maestro_models::PayloadHookInput {
                    payload,
                    signal: hook_signal,
                }
            })))
        },
    )
}
#[cfg(not(target_arch = "wasm32"))]
/// Response hook records the response after endpoint preparation.
fn response_hook(case: &Value, log: &Log, signal: Cancellation) -> OnResponse {
    let (recorded, hook, cancel) = (
        Arc::clone(&log.hooks),
        case["hook"].as_str().unwrap_or_default().to_owned(),
        case["cancel"] == "response",
    );
    Arc::new(move |response, _| {
        let observation = json!(["response", response]);
        recorded
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(observation);
        if cancel {
            signal.abort();
        }
        Box::pin(std::future::ready(if hook == "response_failure" {
            Err(failure("response failed"))
        } else {
            Ok(())
        }))
    })
}
#[cfg(not(target_arch = "wasm32"))]
/// Attach provider-specific transport and hooks to the common options.
fn common(case: &Value, log: &Log) -> TestResult<StreamOptions> {
    let mut options = common_options(case)?;
    let signal = options.signal.clone().ok_or("fixture signal")?;
    options.fetch = Some(fetch(case, log, signal.clone())?);
    options.on_payload = Some(payload_hook(case, log));
    options.on_response = Some(response_hook(case, log, signal));
    Ok(options)
}
#[cfg(not(target_arch = "wasm32"))]
/// The three request states of the tier field.
fn tier(value: &Value) -> Option<OpenAIResponsesServiceTier> {
    match value.as_str() {
        Some("auto") => Some(OpenAIResponsesServiceTier::Auto),
        Some("default") => Some(OpenAIResponsesServiceTier::Default),
        Some("flex") => Some(OpenAIResponsesServiceTier::Flex),
        Some("scale") => Some(OpenAIResponsesServiceTier::Scale),
        Some("priority") => Some(OpenAIResponsesServiceTier::Priority),
        _ => None,
    }
}
#[cfg(not(target_arch = "wasm32"))]
/// Choose the public raw or simple entry point.
fn start(
    case: &Value,
    common: StreamOptions,
) -> TestResult<Result<AssistantMessageEventStream, String>> {
    let spec = &case["options"];
    if case["entry"] == "simple" {
        return Ok(stream_simple_openai_responses(
            target(case)?,
            history(case)?,
            Some(maestro_models::SimpleStreamOptions {
                common,
                reasoning: option(spec, "reasoning")?,
                ..Default::default()
            }),
        )
        .map_err(|e| e.message));
    }
    let reasoning_summary = reasoning_summary(&spec["reasoningSummary"]);
    Ok(Ok(stream_openai_responses(
        target(case)?,
        history(case)?,
        Some(OpenAIResponsesOptions {
            common,
            service_tier: spec.get("serviceTier").map(tier),
            reasoning_effort: option(spec, "reasoningEffort")?,
            reasoning_summary,
        }),
    )))
}
#[cfg(not(target_arch = "wasm32"))]
/// Shared output identity observations.
pub use crate::response_output::collect;
#[cfg(not(target_arch = "wasm32"))]
/// Run one operation until its finite source and stream close.
pub async fn run_case(case: &Value) -> TestResult<Value> {
    let log = Log::default();
    let stream = match start(case, common(case, &log)?)? {
        Ok(s) => s,
        Err(e) => return Ok(json!({"thrown":e})),
    };
    observe(&stream, &log).await
}
#[cfg(not(target_arch = "wasm32"))]
/// Run each query in an isolated child with its explicit environment.
pub async fn assert_rows(test: &str) -> TestResult {
    let decoded = crate::response_cases::rows(FIXTURE)?;
    let rows = crate::chat::rows(&serde_json::to_string(&decoded)?, test)?;
    for row in selected(test, &rows, crate::environment_rows::json_environment)? {
        compare(
            row,
            run_case(&row["case"]).await?,
            &json!({"stream":true,"marker":"replacement"}),
        );
    }
    Ok(())
}
