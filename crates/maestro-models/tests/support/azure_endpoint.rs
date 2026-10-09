//! Controlled public response invocations with finite producers and consumed observations.
use crate::chat::{TestResult, context, model};
use crate::child_process::{child_case, rerun};
use maestro_models::providers::responses::azure_openai_responses::{
    stream_azure_openai_responses, stream_simple_azure_openai_responses,
};
use maestro_models::providers::responses::openai_responses::OpenAIResponsesReasoningSummary;
use maestro_models::{
    AssistantMessageEventStream, AzureOpenAIResponsesOptions, Cancellation, Fetch, FetchError,
    HttpResponse, OnPayload, OnResponse, StreamOptions,
};
use serde_json::{Value, json};
use std::fmt::Write as _;
use std::sync::{Arc, Mutex, PoisonError};

/// Reduced recorded cases.
pub const FIXTURE: &str = include_str!("../fixtures/azure_endpoint_cases.json");
/// Finish event used by request-only cases.
pub const COMPLETE: &str = "data: {\"type\":\"response.completed\",\"response\":{\"id\":\"r-final\",\"status\":\"completed\",\"usage\":{\"input_tokens\":1000,\"output_tokens\":80,\"total_tokens\":1080,\"input_tokens_details\":{\"cached_tokens\":200}}}}\n\n";
/// Model with the recording's rates and limits.
fn target(case: &Value) -> TestResult<maestro_models::Model> {
    let mut descriptor = json!({"id":"gpt-4o-mini","name":"Controlled","api":"azure-openai-responses","provider":"azure-openai-responses","baseUrl":"https://controlled.invalid/v1","cost":{"input":2,"output":7,"cacheRead":0.3,"cacheWrite":0.9},"contextWindow":100_000,"maxTokens":64_000});
    if let Some(changes) = case["model"].as_object() {
        descriptor
            .as_object_mut()
            .ok_or("descriptor object")?
            .extend(changes.clone());
    }
    model(&descriptor)
}
/// Conversation defaults overridden by a selected fixture history.
fn history(case: &Value) -> TestResult<maestro_models::Context> {
    let mut history =
        json!({"systemPrompt":"instruction","messages":[{"role":"user","content":"query"}]});
    if let Some(changes) = case["context"].as_object() {
        history
            .as_object_mut()
            .ok_or("history object")?
            .extend(changes.clone());
    }
    context(&history)
}
/// Read a tagged nonfinite test number.
fn number(value: &Value) -> Option<f64> {
    match value["number"].as_str() {
        Some("NaN") => Some(f64::NAN),
        Some("Infinity") => Some(f64::INFINITY),
        _ => value.as_f64(),
    }
}
/// Decode an optional typed fixture option.
fn option<T: serde::de::DeserializeOwned>(value: &Value, key: &str) -> TestResult<Option<T>> {
    Ok(value
        .get(key)
        .filter(|v| !v.is_null())
        .map(|v| serde_json::from_value(v.clone()))
        .transpose()?)
}
/// Hook failure returned as data.
fn failure(text: &str) -> maestro_models::DiagnosticErrorInfo {
    maestro_models::extract_diagnostic_error(maestro_models::DiagnosticInput::Text(text))
}
/// Script the finite response body.
fn body(case: &Value) -> Vec<u8> {
    if let Some(body) = case["body"].as_str() {
        return body.as_bytes().to_vec();
    }
    let Some(events) = case["events"].as_array() else {
        return COMPLETE.as_bytes().to_vec();
    };
    events
        .iter()
        .fold(String::new(), |mut s, e| {
            let _ = write!(s, "data: {e}\n\n");
            s
        })
        .into_bytes()
}
/// Finite observation logs containing only owned JSON data.
#[derive(Clone, Default)]
struct Log {
    /// Captured wire requests.
    requests: Arc<Mutex<Vec<Value>>>,
    /// Payload/response hook observations.
    hooks: Arc<Mutex<Vec<Value>>>,
}
/// Controlled transport; body construction never runs under a log lock.
fn fetch(case: &Value, log: &Log, signal: Cancellation) -> TestResult<Fetch> {
    let (sent, wire, status, fragmented) = (
        Arc::clone(&log.requests),
        body(case),
        u16::try_from(case["status"].as_u64().unwrap_or(200))?,
        case["chunked"] == true,
    );
    let body_abort = case["cancel"] == "body";
    let body_failure = case["cancel"] == "body_failure";
    let content_type = case["contentType"]
        .as_str()
        .unwrap_or("text/event-stream")
        .to_owned();
    Ok(Arc::new(move |request| {
        let mut captured = json!({"url":request.url,"headers":request.headers,"body":serde_json::from_slice::<Value>(&request.body).unwrap_or(Value::Null)});
        {
            captured["bodyKeys"] = json!(
                captured["body"]
                    .as_object()
                    .map(|body| body.keys().collect::<Vec<_>>())
            );
        }
        sent.lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(captured);
        let chunks = if body_failure {
            signal.abort();
            vec![Err(FetchError::Connection(failure("body failed")))]
        } else if body_abort {
            signal.abort();
            vec![Err(FetchError::Aborted)]
        } else if fragmented {
            wire.chunks(3).map(|c| Ok(c.to_vec())).collect()
        } else {
            vec![Ok(wire.clone())]
        };
        Box::pin(std::future::ready(Ok(HttpResponse {
            status,
            status_text: String::new(),
            headers: [
                ("content-type".into(), content_type.clone()),
                ("x-controlled".into(), "yes".into()),
            ]
            .into(),
            body: Box::pin(futures_util::stream::iter(chunks)),
        })))
    }))
}
/// Payload edits reach the transport, and failures prevent it.
fn payload_hook(case: &Value, log: &Log) -> OnPayload {
    let (recorded, hook) = (
        Arc::clone(&log.hooks),
        case["hook"].as_str().unwrap_or_default().to_owned(),
    );
    Arc::new(move |mut payload, model| {
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
            "replace" => {
                Ok(json!({"model":"hook-model","input":[],"stream":true,"marker":"replacement"}))
            }
            "mutate" => {
                payload["model"] = json!("mutated-model");
                Ok(payload)
            }
            _ => Ok(payload),
        };
        Box::pin(std::future::ready(result))
    })
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
/// Common option projection from the reduced recording.
fn common(case: &Value, log: &Log) -> TestResult<StreamOptions> {
    let signal = Cancellation::new();
    if case["cancel"] == "before" {
        signal.abort();
    }
    let spec = &case["options"];
    let api_key = if case["missingKey"] == true {
        None
    } else {
        Some(spec["apiKey"].as_str().unwrap_or("test-key").to_owned())
    };
    Ok(StreamOptions {
        api_key,
        max_retries: Some(spec["maxRetries"].as_f64().unwrap_or(0.0)),
        signal: Some(signal.clone()),
        max_tokens: number(&spec["maxTokens"]),
        temperature: number(&spec["temperature"]),
        session_id: spec["sessionId"].as_str().map(str::to_owned),
        cache_retention: option(spec, "cacheRetention")?,
        headers: option(spec, "headers")?,
        transport: option(spec, "transport")?,
        metadata: option(spec, "metadata")?,
        max_retry_delay_ms: spec["maxRetryDelayMs"].as_f64(),
        timeout_ms: spec["timeoutMs"].as_f64(),
        fetch: Some(fetch(case, log, signal.clone())?),
        on_payload: Some(payload_hook(case, log)),
        on_response: Some(response_hook(case, log, signal)),
    })
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
    let reasoning_summary = match spec["reasoningSummary"].as_str() {
        Some("auto") => Some(OpenAIResponsesReasoningSummary::Auto),
        Some("detailed") => Some(OpenAIResponsesReasoningSummary::Detailed),
        Some("concise") => Some(OpenAIResponsesReasoningSummary::Concise),
        _ => None,
    };
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
    let (events, result) = crate::response_output::collect(&stream).await?;
    if case["status"] == 400 {
        let message = stream.result().await;
        assert!(maestro_models::is_context_overflow(
            &*message.read().map_err(|e| e.to_string())?,
            None
        ));
    }
    let requests = log.requests.lock().map_err(|e| e.to_string())?.clone();
    let hooks = log.hooks.lock().map_err(|e| e.to_string())?.clone();
    Ok(json!({"requests":requests,"hooks":hooks,"events":events,"result":result}))
}
/// Compare recorded observations, with native parser text checked at its own boundary.
fn compare(row: &Value, mut actual: Value) {
    if row["case"]["hook"] == "replace" {
        assert_eq!(
            actual["requests"][0]["body"],
            json!({"model":"hook-model","input":[],"stream":true,"marker":"replacement"})
        );
    }
    if row["expected"]["result"]["errorMessage"] == json!({"nativeDiagnostic":true}) {
        assert!(
            actual["result"]["errorMessage"]
                .as_str()
                .is_some_and(|s| !s.is_empty())
        );
        actual["result"]["errorMessage"] = json!({"nativeDiagnostic":true});
    }
    assert_eq!(
        crate::json::canonical(actual),
        crate::json::canonical(row["expected"].clone()),
        "{}",
        row["case"]
    );
}
/// Environment variables named by the scenario.
fn environment(case: &Value) -> TestResult<Vec<(&str, &str)>> {
    case["env"]
        .as_object()
        .into_iter()
        .flatten()
        .map(|(k, v)| Ok((k.as_str(), v.as_str().ok_or("environment value")?)))
        .collect()
}
/// Run each query in an isolated child with its explicit environment.
pub async fn assert_rows(test: &str) -> TestResult {
    let decoded = crate::azure_cases::rows(FIXTURE)?;
    let rows = crate::chat::rows(&serde_json::to_string(&decoded)?, test)?;
    assert!(!rows.is_empty(), "{test} owns cases");
    let Some(selected) = child_case() else {
        rerun(test, "*", &[])?;
        for (index, row) in rows
            .iter()
            .enumerate()
            .filter(|(_, row)| row["case"].get("env").is_some())
        {
            rerun(test, &index.to_string(), &environment(&row["case"])?)?;
        }
        return Ok(());
    };
    for (index, row) in rows.iter().enumerate() {
        if (selected == "*" && row["case"].get("env").is_none()) || selected == index.to_string() {
            compare(row, run_case(&row["case"]).await?);
        }
    }
    Ok(())
}
