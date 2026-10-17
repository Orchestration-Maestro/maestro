//! Recorded invocation inputs and complete public observations.
use crate::{chat, child_process};
use chat::TestResult;
use maestro_models::{
    Cancellation, Context, DiagnosticErrorInfo, FetchError, HttpRequest, HttpResponse,
    ProviderStreamOptions, SimpleStreamOptions, StreamOptions,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// One independently recorded case.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    /// Owning test.
    test: String,
    /// Complete authored input.
    input: Input,
    /// Complete expected output tree.
    expected: Value,
}

/// Authored input accepted by the invocation oracle.
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Input {
    /// Text body chunks.
    chunks: Option<Vec<String>>,
    /// Binary body chunks.
    hex_chunks: Option<Vec<String>>,
    /// Hook action.
    hook: Option<String>,
    /// Omit the hook.
    no_hook: Option<bool>,
    /// Supplied key.
    api_key: Option<String>,
    /// Environmental key.
    env_key: Option<String>,
    /// Use the simple entry.
    simple: Option<bool>,
    /// Common options.
    common: Option<Common>,
    /// Install a response observer.
    response_hook: Option<bool>,
    /// Hook HTTP controls.
    http_options: Option<HttpOptions>,
    /// Response status.
    status: Option<u16>,
    /// Response reason phrase.
    status_text: Option<String>,
    /// Response media type.
    content_type: Option<String>,
    /// Fetch failure.
    fetch_error: Option<String>,
    /// Body failure.
    body_error: Option<String>,
    /// Caller cancellation phase.
    abort: Option<String>,
    /// Model identity.
    model_id: Option<String>,
    /// Authored model endpoint.
    base_url: Option<String>,
    /// Simple reasoning.
    reasoning: Option<maestro_models::ThinkingLevel>,
    /// Raw tool choice.
    tool_choice: Option<String>,
    /// Raw thinking.
    thinking: Option<Thinking>,
    /// Include tools.
    tools: Option<bool>,
    /// Public endpoint fallback.
    env_base: Option<String>,
    /// Cloud-only endpoint.
    env_vertex_base: Option<String>,
}

/// Only the common fixture fields materialized as native stream options.
#[derive(Clone, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Common {
    /// Temperature forwarded by simple selection.
    temperature: Option<f64>,
    /// Token limit forwarded by simple selection.
    max_tokens: Option<f64>,
    /// Unused common deadline.
    timeout_ms: Option<f64>,
    /// Unused retry count.
    max_retries: Option<f64>,
    /// Unused retry delay.
    max_retry_delay_ms: Option<f64>,
    /// Unused transport preference.
    transport: Option<maestro_models::Transport>,
}

/// Hook controls represented in the corpus.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct HttpOptions {
    /// Selected milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    timeout: Option<f64>,
    /// Request-local retry settings, unused by this provider.
    #[serde(skip_serializing_if = "Option::is_none")]
    retry_options: Option<RetryOptions>,
}

/// Request-local attempts carried through the selected payload.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RetryOptions {
    /// Authored attempt count.
    attempts: f64,
}

/// Declared thinking fields used by recorded valid inputs.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Thinking {
    /// Enabled or disabled thinking.
    enabled: bool,
    /// Optional budget.
    #[serde(skip_serializing_if = "Option::is_none")]
    budget_tokens: Option<f64>,
    /// Optional level.
    #[serde(skip_serializing_if = "Option::is_none")]
    level: Option<String>,
}

/// Test-owned observations captured before completing the transport.
#[derive(Default)]
struct Observed {
    /// Request snapshots.
    requests: Mutex<Vec<HttpRequest>>,
    /// Payload callbacks.
    hooks: AtomicUsize,
    /// Response callbacks.
    response_hooks: AtomicUsize,
}

/// Materialize bytes without dropping invalid UTF-8 samples.
fn chunks(input: &Input) -> TestResult<Vec<Vec<u8>>> {
    if let Some(hex) = &input.hex_chunks {
        return hex
            .iter()
            .map(|text| {
                assert_eq!(text.len() % 2, 0);
                (0..text.len())
                    .step_by(2)
                    .map(|i| u8::from_str_radix(&text[i..i + 2], 16).map_err(Into::into))
                    .collect()
            })
            .collect();
    }
    Ok(input
        .chunks
        .as_deref()
        .unwrap_or_default()
        .iter()
        .map(|text| text.as_bytes().to_vec())
        .collect())
}

/// Capture a single attempt and return the controlled response source.
fn fetch(
    input: &Input,
    observed: Arc<Observed>,
    signal: Cancellation,
) -> TestResult<maestro_models::Fetch> {
    let chunks = chunks(input)?;
    let status = input.status.unwrap_or(200);
    let status_text = input.status_text.clone().unwrap_or_default();
    let headers = BTreeMap::from([(
        "content-type".into(),
        input
            .content_type
            .clone()
            .unwrap_or_else(|| "text/event-stream".into()),
    )]);
    let failure = input.fetch_error.clone();
    let body_error = input.body_error.clone();
    let abort = input.abort.as_deref() == Some("body");
    Ok(Arc::new(move |request| {
        observed.requests.lock().unwrap().push(request);
        let (chunks, headers, status_text, failure, body_error, signal) = (
            chunks.clone(),
            headers.clone(),
            status_text.clone(),
            failure.clone(),
            body_error.clone(),
            signal.clone(),
        );
        Box::pin(async move {
            if let Some(message) = failure {
                return Err(FetchError::Connection(DiagnosticErrorInfo::error(message)));
            }
            let mut items: Vec<_> = chunks.into_iter().map(Ok).collect();
            if abort {
                // The abort occurs only when the body is polled after its recorded chunks.
                let body = futures_util::stream::iter(items)
                    .chain(futures_util::stream::once(abort_body(signal)));
                return Ok(HttpResponse {
                    status,
                    status_text,
                    headers,
                    body: Box::pin(body),
                });
            }
            if let Some(message) = body_error {
                items.push(Err(FetchError::Connection(DiagnosticErrorInfo::error(
                    message,
                ))));
            }
            Ok(HttpResponse {
                status,
                status_text,
                headers,
                body: Box::pin(futures_util::stream::iter(items)),
            })
        })
    }))
}

use futures_util::StreamExt;

/// Install the recorded hook, returning its owned selected payload.
fn payload_hook(
    input: &Input,
    observed: Arc<Observed>,
    signal: Cancellation,
) -> Option<maestro_models::OnPayload> {
    if input.no_hook == Some(true) {
        return None;
    }
    let action = input.hook.clone();
    let controls = input.http_options.clone();
    let abort = input.abort.as_deref() == Some("hook");
    Some(Arc::new(
        move |maestro_models::PayloadHookInput {
                  mut payload,
                  signal: mut hook_signal,
              },
              _| {
            observed.hooks.fetch_add(1, Ordering::Relaxed);
            if abort {
                signal.abort();
            }
            if let Some(controls) = &controls {
                payload["config"]["httpOptions"] = serde_json::to_value(controls).unwrap();
            }
            if matches!(
                action.as_deref(),
                Some("replace" | "bad-model" | "remove-signal")
            ) {
                hook_signal = None;
            }
            let result = match action.as_deref() {
                Some("throw") => Err(DiagnosticErrorInfo::error("controlled payload failure")),
                Some("replace") => Ok(
                    json!({"model":"replacement", "contents":"replacement prompt", "config":{"temperature":0.75}}),
                ),
                Some("bad-model") => Ok(json!({"model":"bad..model", "contents":"prompt"})),
                Some("remove-signal") => {
                    payload["config"]
                        .as_object_mut()
                        .unwrap()
                        .shift_remove("abortSignal");
                    Ok(payload)
                }
                Some("edit") => {
                    payload["config"]["temperature"] = json!(0.375);
                    Ok(payload)
                }
                Some("undefined") => {
                    payload["config"]["temperature"] = json!(0.625);
                    Ok(payload)
                }
                None => Ok(payload),
                _ => panic!("unknown hook action"),
            };
            Box::pin(async move {
                result.map(|payload| maestro_models::PayloadHookInput {
                    payload,
                    signal: hook_signal,
                })
            })
        },
    ))
}

/// Selected common settings and native handles.
fn common(
    input: &Input,
    observed: &Arc<Observed>,
    signal: &Cancellation,
) -> TestResult<StreamOptions> {
    let value = input.common.clone().unwrap_or_default();
    let mut options = StreamOptions::<maestro_models::Threaded> {
        temperature: value.temperature,
        max_tokens: value.max_tokens,
        timeout_ms: value.timeout_ms,
        max_retries: value.max_retries,
        max_retry_delay_ms: value.max_retry_delay_ms,
        transport: value.transport,
        ..Default::default()
    };
    options.api_key = Some(
        input
            .api_key
            .clone()
            .unwrap_or_else(|| "fixture-key".into()),
    );
    options.signal = input.abort.as_ref().map(|_| signal.clone());
    options.fetch = Some(fetch(input, Arc::clone(observed), signal.clone())?);
    options.on_payload = payload_hook(input, Arc::clone(observed), signal.clone());
    if input.response_hook == Some(true) {
        let observed = Arc::clone(observed);
        options.on_response = Some(Arc::new(move |_, _| {
            observed.response_hooks.fetch_add(1, Ordering::Relaxed);
            Box::pin(async { Err(DiagnosticErrorInfo::error("response hook must not run")) })
        }));
    }
    Ok(options)
}

/// Drive one row through the actual public invocation.
async fn observe(input: &Input) -> TestResult<Value> {
    let signal = Cancellation::new();
    if input.abort.as_deref() == Some("before") {
        signal.abort();
    }
    let observed = Arc::new(Observed::default());
    let model = chat::model(
        &json!({"id":input.model_id.as_deref().unwrap_or("gemini-2.5-flash"), "name":"fixture", "api":"google-generative-ai", "provider":"google", "baseUrl":input.base_url.as_deref().unwrap_or("https://fixture.invalid/v1beta"), "maxTokens":8192, "cost":{"input":1,"output":2,"cacheRead":3,"cacheWrite":4}}),
    )?;
    let mut context = json!({"systemPrompt":"changed system", "messages":[{"role":"user","content":"changed prompt"}]});
    if input.tools == Some(true) {
        context["tools"] = json!([{"name":"lookup", "description":"Find changed record", "parameters":{"type":"object", "properties":{"q":{"type":"string"}}}}]);
    }
    let context: Context = chat::context(&context)?;
    let common = common(input, &observed, &signal)?;
    let result = if input.simple == Some(true) {
        maestro_models::providers::content::google::stream_simple_google(
            model,
            context,
            Some(SimpleStreamOptions {
                common,
                reasoning: input.reasoning,
                ..Default::default()
            }),
        )
    } else {
        let extra = json!({"toolChoice":input.tool_choice, "thinking":input.thinking})
            .as_object()
            .unwrap()
            .clone();
        Ok(maestro_models::stream(
            model,
            context,
            Some(ProviderStreamOptions {
                common,
                extra,
                ..Default::default()
            }),
        )?)
    };
    let (events, output, thrown) = settled(result).await?;
    let mut result = json!({"fetches":observed.requests.lock().unwrap().len(), "hooks":observed.hooks.load(Ordering::Relaxed), "responseHooks":observed.response_hooks.load(Ordering::Relaxed), "events":events});
    if let Some(value) = output {
        result["output"] = value;
    }
    if let Some(value) = thrown {
        result["thrown"] = value;
    }
    if let Some(request) = observed.requests.lock().unwrap().first() {
        result["request"] = json!({"url":request.url, "method":request.method, "body":String::from_utf8(request.body.clone())?, "headers":request.headers.iter().collect::<Vec<_>>(), "hasSignal":request.signal.is_some()});
    }
    Ok(result)
}

/// Compare expected fields after normalizing native diagnostics and JSON numeric representation.
fn check(row: &Row) -> TestResult {
    let mut actual = chat::block_on(false, observe(&row.input))?;
    if let Some(expected) = row.expected["output"]["errorMessage"]
        .as_str()
        .filter(|text| text.starts_with('<'))
    {
        let message = actual["output"]["errorMessage"]
            .as_str()
            .expect("diagnostic required");
        match expected {
            "<URL failure>" => assert!(message.starts_with("URL:"), "URL failure kind required"),
            "<JSON parse failure>" => assert!(
                [
                    "EOF while parsing",
                    "expected value",
                    "expected ident",
                    "trailing characters",
                    "key must be a string"
                ]
                .iter()
                .any(|prefix| message.starts_with(prefix)),
                "native JSON parse failure kind required"
            ),
            _ => return Err("unknown diagnostic normalization".into()),
        }
        actual["output"]["errorMessage"] = json!(expected);
    }
    let actual: Value = serde_json::from_str(&maestro_models::compact_json(&actual)?)?;
    let expected: Value = serde_json::from_str(&maestro_models::compact_json(&row.expected)?)?;
    assert!(
        actual == expected,
        "case output differs: test={} events={} output={}",
        row.test,
        actual["events"],
        actual["output"]
    );
    Ok(())
}

/// Evaluate named rows in isolated processes with their specified environment.
pub fn assert_rows(test: &str) -> TestResult {
    let rows: Vec<Row> = serde_json::from_str(include_str!(
        "../fixtures/content_generation_invocation.json"
    ))?;
    if let Some(case) = child_process::child_case() {
        let row = &rows[case.parse::<usize>()?];
        assert_eq!(row.test, test);
        return check(row);
    }
    let mut count = 0;
    for (index, row) in rows.iter().enumerate().filter(|(_, row)| row.test == test) {
        count += 1;
        let mut env = Vec::new();
        for (key, value) in [
            ("GEMINI_API_KEY", &row.input.env_key),
            ("GOOGLE_GEMINI_BASE_URL", &row.input.env_base),
            ("GOOGLE_VERTEX_BASE_URL", &row.input.env_vertex_base),
        ] {
            if let Some(value) = value {
                env.push((key, value.as_str()));
            }
        }
        child_process::rerun(test, &index.to_string(), &env)?;
    }
    assert!(count > 0, "no fixture rows");
    Ok(())
}

/// Consume terminal output without retaining emission-time snapshots of live aliases.
async fn settled(
    result: Result<maestro_models::AssistantMessageEventStream, DiagnosticErrorInfo>,
) -> TestResult<(Vec<Value>, Option<Value>, Option<Value>)> {
    let mut events = Vec::new();
    let mut output = None;
    let mut thrown = None;
    match result {
        Err(error) => thrown = Some(json!({"name":"Error", "message":error.message})),
        Ok(stream) => {
            let (seen, result) = crate::response_output::collect(&stream).await?;
            events = seen;
            output = Some(result);
        }
    }
    Ok((events, output, thrown))
}

/// Abort at the next body read rather than at response construction.
fn abort_body(
    signal: Cancellation,
) -> impl std::future::Future<Output = Result<Vec<u8>, FetchError>> {
    std::future::poll_fn(move |_| {
        signal.abort();
        std::task::Poll::Ready(Err(FetchError::Connection(DiagnosticErrorInfo::error(
            "The operation was aborted.",
        ))))
    })
}
