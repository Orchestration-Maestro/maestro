//! Controlled public response invocations with finite producers and consumed observations.
use crate::chat::TestResult;
use crate::environment_rows::selected;
use crate::response_support::{
    Log, body_keys, common as common_options, compare, failure, fetch as shared_fetch, history,
    observe, option, reasoning_summary, request, target as target_with_descriptor,
};
use maestro_models::providers::responses::azure_openai_responses::{
    stream_azure_openai_responses, stream_simple_azure_openai_responses,
};
use maestro_models::{
    AssistantMessageEventStream, AzureOpenAIResponsesOptions, Cancellation, Fetch, OnPayload,
    OnResponse, StreamOptions,
};
use serde_json::{Value, json};
use std::sync::{Arc, PoisonError};

/// Reduced recorded cases.
pub const FIXTURE: &str = include_str!("../fixtures/azure_endpoint_cases.json");
/// Finish event used by request-only cases.
pub const COMPLETE: &str = "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"r-final\",\"status\":\"completed\",\"usage\":{\"input_tokens\":1000,\"output_tokens\":80,\"total_tokens\":1080,\"input_tokens_details\":{\"cached_tokens\":200}}}}\n\n";
/// Model with the recording's rates and limits.
fn target(case: &Value) -> TestResult<maestro_models::Model> {
    target_with_descriptor(
        case,
        json!({"id":"gpt-4o-mini","name":"Controlled","api":"azure-openai-responses","provider":"azure-openai-responses","baseUrl":"https://controlled.invalid/v1","cost":{"input":2,"output":7,"cacheRead":0.3,"cacheWrite":0.9},"contextWindow":100_000,"maxTokens":64_000}),
    )
}
/// Controlled transport with provider-specific wire observations.
fn fetch(case: &Value, log: &Log, signal: Cancellation) -> TestResult<Fetch> {
    shared_fetch(case, log, signal, COMPLETE, move |request_value| {
        let mut captured = request(request_value);
        body_keys(&mut captured);
        captured
    })
}
/// Payload edits reach the transport, and failures prevent it.
fn payload_hook(case: &Value, log: &Log) -> OnPayload {
    let (recorded, hook) = (
        Arc::clone(&log.hooks),
        case["hook"].as_str().unwrap_or_default().to_owned(),
    );
    Arc::new(
        move |maestro_models::PayloadHookInput {
                  mut payload,
                  signal: hook_signal,
              },
              model| {
            let label = json!({"kind":"payload","model":model.id,"keys":payload.as_object().map(|v|v.keys().collect::<Vec<_>>()),"payload":payload});
            recorded
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(label);
            let result = match hook.as_str() {
                "payload_failure" => Err(failure("payload failed")),
                "nonstream" => {
                    payload["stream"] = json!(false);
                    Ok(payload)
                }
                "replace" => Ok(
                    json!({"model":"hook-model","input":[],"stream":true,"marker":"replacement"}),
                ),
                "mutate" => {
                    payload["model"] = json!("mutated-model");
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
/// Response hook records the response after endpoint preparation.
fn response_hook(case: &Value, log: &Log, signal: Cancellation) -> OnResponse {
    let (recorded, hook, cancel) = (
        Arc::clone(&log.hooks),
        case["hook"].as_str().unwrap_or_default().to_owned(),
        case["cancel"] == "response",
    );
    Arc::new(move |response, model| {
        let observation = json!({"kind":"response","model":model.id,"response":response});
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
/// Attach provider-specific transport and hooks to the common options.
fn common(case: &Value, log: &Log) -> TestResult<StreamOptions> {
    let mut options = common_options(case)?;
    let signal = options.signal.clone().ok_or("fixture signal")?;
    options.timeout_ms = case["options"]["timeoutMs"].as_f64();
    options.fetch = Some(fetch(case, log, signal.clone())?);
    options.on_payload = Some(payload_hook(case, log));
    options.on_response = Some(response_hook(case, log, signal));
    Ok(options)
}
/// Choose the public raw or simple entry point.
fn start(
    case: &Value,
    common: StreamOptions,
) -> TestResult<Result<AssistantMessageEventStream, String>> {
    let spec = &case["options"];
    if case["entry"] == "simple" {
        return Ok(stream_simple_azure_openai_responses(
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
    Ok(Ok(stream_azure_openai_responses(
        target(case)?,
        history(case)?,
        Some(AzureOpenAIResponsesOptions {
            common,
            azure_base_url: option(spec, "azureBaseUrl")?,
            azure_resource_name: option(spec, "azureResourceName")?,
            azure_api_version: option(spec, "azureApiVersion")?,
            azure_deployment_name: option(spec, "azureDeploymentName")?,
            reasoning_effort: option(spec, "reasoningEffort")?,
            reasoning_summary,
        }),
    )))
}
/// Run one operation until its finite source and stream close.
pub async fn run_case(case: &Value) -> TestResult<Value> {
    let log = Log::default();
    let stream = match start(case, common(case, &log)?)? {
        Ok(s) => s,
        Err(e) => return Ok(json!({"thrown":e})),
    };
    let actual = observe(&stream, &log).await?;
    if case["status"] == 400 {
        let message = stream.result().await;
        assert!(maestro_models::is_context_overflow(
            &*message.read().map_err(|e| e.to_string())?,
            None
        ));
    }
    Ok(actual)
}
/// Run each query in an isolated child with its explicit environment.
pub async fn assert_rows(test: &str) -> TestResult {
    let decoded = crate::azure_cases::rows(FIXTURE)?;
    let rows = crate::chat::rows(&serde_json::to_string(&decoded)?, test)?;
    for row in selected(
        test,
        &rows,
        |index, _| Ok(index.to_string()),
        crate::environment_rows::json_environment,
    )? {
        compare(
            row,
            run_case(&row["case"]).await?,
            &json!({"model":"hook-model","input":[],"stream":true,"marker":"replacement"}),
        );
    }
    Ok(())
}
