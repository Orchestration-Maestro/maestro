//! Strict recorded inputs and full cloud-entry observations.
#![cfg(test)]
use crate::{chat, content_credentials, loopback, response_output};
use chat::TestResult;
use maestro_models::{
    Cancellation, Context, DiagnosticErrorInfo, FetchError, HttpRequest, HttpResponse,
    ProviderStreamOptions, SimpleStreamOptions, StreamOptions,
};
use maestro_request::presence::Presence;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

/// One recorded entry invocation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    /// Test owning the observation.
    test: String,
    /// Authored entry inputs.
    input: Input,
    /// Recorded output tree, compared in full.
    expected: Value,
}

/// Complete native inputs admitted by the recorder.
#[derive(Clone, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Input {
    /// Already-aborted caller.
    abort_before: Option<bool>,
    /// Credential file created in the payload hook.
    adc_appears_inside_hook: Option<bool>,
    /// Supplied provider key.
    api_key: Option<String>,
    /// Authored endpoint.
    base_url: Option<String>,
    /// Public or cloud entry.
    entry: Option<String>,
    /// Explicit child environment.
    env: Option<BTreeMap<String, String>>,
    /// Transport/body failure.
    failure: Option<String>,
    /// Invocation headers.
    headers: Option<indexmap::IndexMap<String, String>>,
    /// Payload callback action.
    hook: Option<String>,
    /// Open per-request controls.
    http_options: Option<Value>,
    /// Model identifier.
    id: Option<String>,
    /// Raw location override.
    #[serde(default)]
    location: Presence<String>,
    /// Model headers.
    model_headers: Option<indexmap::IndexMap<String, String>>,
    /// Omit default raw scope.
    no_scope: Option<bool>,
    /// Raw project override.
    #[serde(default)]
    project: Presence<String>,
    /// Simple reasoning request.
    reasoning: Option<maestro_models::ThinkingLevel>,
    /// Whether the model supports thinking.
    reasoning_model: Option<bool>,
    /// Simple rather than raw entry.
    simple: Option<bool>,
    /// Raw thinking controls.
    thinking: Option<Thinking>,
    /// Caller-supplied simple budgets.
    thinking_budgets: Option<maestro_models::ThinkingBudgets>,
    /// Raw tool mode.
    tool_choice: Option<String>,
    /// Supply a declaration.
    tools: Option<bool>,
}

/// The recorded raw thinking object.
#[derive(Clone, Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Thinking {
    /// Whether thinking is enabled.
    enabled: bool,
    /// Optional budget.
    #[serde(skip_serializing_if = "Option::is_none")]
    budget_tokens: Option<f64>,
    /// Optional family level.
    #[serde(skip_serializing_if = "Option::is_none")]
    level: Option<maestro_models::GoogleThinkingLevel>,
}

/// Closed observations, inspected only after the producer settles.
#[derive(Default)]
struct Observed {
    /// Ordered sent requests.
    requests: Mutex<Vec<HttpRequest>>,
    /// Payload calls.
    hooks: AtomicUsize,
    /// Response calls.
    responses: AtomicUsize,
    /// Selected signal at dispatch.
    selected_aborted: Mutex<Option<bool>>,
}

/// Build the actual public model and context, not an echoed fixture projection.
fn conversation(input: &Input) -> TestResult<(maestro_models::Model, Context)> {
    let public = input.entry.as_deref() == Some("public");
    let model = chat::model(&json!({
        "id":input.id.as_deref().unwrap_or("gemini-2.5-flash"), "name":"fixture",
        "api":if public {"google-generative-ai"} else {"google-vertex"},
        "provider":if public {"google"} else {"google-vertex"},
        "baseUrl":input.base_url.as_deref().unwrap_or(if public {"https://generativelanguage.googleapis.com/v1beta"} else {"https://{location}-aiplatform.googleapis.com"}),
        "reasoning":input.reasoning_model.unwrap_or(true),"input":["text"],
        "cost":{"input":1,"output":2,"cacheRead":0.5,"cacheWrite":0},"contextWindow":128,"maxTokens":64,
        "headers":input.model_headers
    }))?;
    let mut context =
        json!({"messages":[{"role":"user","content":"before-hook"}],"systemPrompt":"system"});
    if input.tools == Some(true) {
        context["tools"] = json!([{"name":"lookup","description":"find","parameters":{"type":"object","properties":{"q":{"type":"string"}}}}]);
    }
    Ok((model, chat::context(&context)?))
}

/// Payload edits and signal selection occur before projection, not in the transport stub.
fn hook(
    input: Input,
    seen: Arc<Observed>,
    original: Cancellation,
    selected: Cancellation,
    credentials: Value,
) -> maestro_models::OnPayload {
    Arc::new(move |mut value, _| {
        seen.hooks.fetch_add(1, Ordering::SeqCst);
        if input.adc_appears_inside_hook == Some(true) {
            write_credentials(&credentials);
        }
        let result = edit(&input, &mut value, &original, &selected);
        Box::pin(async move { result.map(|()| value) })
    })
}

/// Apply the independently recorded callback operation to the actual prepared payload.
fn edit(
    input: &Input,
    value: &mut maestro_models::PayloadHookInput,
    original: &Cancellation,
    selected: &Cancellation,
) -> Result<(), DiagnosticErrorInfo> {
    match input.hook.as_deref() {
        Some("fail") => return Err(DiagnosticErrorInfo::error("controlled payload failure")),
        Some("mutate") => value.payload["contents"] = json!("mutated"),
        Some("replace") => {
            value.payload = json!({"model":"gemini-replaced","contents":"replacement","config":{}});
        }
        Some("delete") => {
            value
                .payload
                .as_object_mut()
                .unwrap()
                .shift_remove("contents");
        }
        Some("add") => value.payload["config"]["seed"] = json!(19),
        Some("abort-original") => original.abort(),
        Some("replace-signal" | "cancel-selected-on-fetch") => {
            value.signal = Some(selected.clone());
        }
        Some("remove-signal") => value.signal = None,
        Some("remove-signal-abort-original") => {
            value.signal = None;
            original.abort();
        }
        Some("abort-selected") => {
            value.signal = Some(selected.clone());
            selected.abort();
        }
        None => {}
        _ => panic!("unknown callback action"),
    }
    if let Some(controls) = &input.http_options {
        value.payload["config"]["httpOptions"] = controls.clone();
    }
    Ok(())
}

/// Create the credential file exclusively at the path selected before child startup.
fn write_credentials(value: &Value) {
    use std::io::Write;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(std::env::var("GOOGLE_APPLICATION_CREDENTIALS").unwrap())
        .unwrap()
        .write_all(&serde_json::to_vec(value).unwrap())
        .unwrap();
}

/// Controlled fetch executes below the shared HTTP/authentication/projection owners.
fn fetch(input: &Input, seen: Arc<Observed>, selected: Cancellation) -> maestro_models::Fetch {
    let failure = input.failure.clone();
    let cancel = input.hook.as_deref() == Some("cancel-selected-on-fetch");
    Arc::new(move |request| {
        *seen.selected_aborted.lock().unwrap() = Some(
            request
                .signal
                .as_ref()
                .is_some_and(Cancellation::is_aborted),
        );
        seen.requests.lock().unwrap().push(request);
        if cancel {
            selected.abort();
        }
        let failure = failure.clone();
        Box::pin(async move {
            if cancel {
                return Err(FetchError::Connection(DiagnosticErrorInfo::error(
                    "controlled selected cancellation",
                )));
            }
            if failure.as_deref() == Some("fetch") {
                return Err(FetchError::Connection(DiagnosticErrorInfo::error(
                    "controlled fetch failure",
                )));
            }
            Ok(reply(failure.as_deref()))
        })
    })
}

/// Run a single child invocation through registered or direct simple dispatch.
async fn observe(input: &Input, credentials: Value, refreshes: &AtomicUsize) -> TestResult<Value> {
    let original = Cancellation::new();
    let selected = Cancellation::new();
    if input.abort_before == Some(true) {
        original.abort();
    }
    let seen = Arc::new(Observed::default());
    let response_seen = Arc::clone(&seen);
    let common = StreamOptions::<maestro_models::Threaded> {
        api_key: input.api_key.clone(),
        temperature: Some(0.0),
        max_tokens: Some(32.0),
        headers: input.headers.clone(),
        signal: Some(original.clone()),
        fetch: Some(fetch(input, Arc::clone(&seen), selected.clone())),
        on_payload: Some(hook(
            input.clone(),
            Arc::clone(&seen),
            original,
            selected,
            credentials,
        )),
        on_response: Some(Arc::new(move |_, _| {
            response_seen.responses.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Ok(()) })
        })),
        ..StreamOptions::default()
    };
    let entry = entry(input, common)?;
    let mut actual = json!({"requests":[],"events":[],"hooks":seen.hooks.load(Ordering::SeqCst),"responseHooks":0,"credentialRefreshes":0});
    match entry {
        Err(error) => actual["thrown"] = json!(error.message),
        Ok(stream) => {
            let (events, result) = response_output::collect(&stream).await?;
            actual["events"] = events
                .into_iter()
                .map(|event| event["type"].clone())
                .collect();
            actual["result"] = result;
        }
    }
    actual["hooks"] = json!(seen.hooks.load(Ordering::SeqCst));
    actual["responseHooks"] = json!(seen.responses.load(Ordering::SeqCst));
    actual["credentialRefreshes"] = json!(refreshes.load(Ordering::SeqCst));
    if let Some(value) = *seen.selected_aborted.lock().unwrap() {
        actual["selectedAborted"] = json!(value);
    }
    actual["requests"] = seen.requests.lock().unwrap().iter().map(|request| json!({"url":request.url,"headers":request.headers,"body":String::from_utf8(request.body.clone()).unwrap()})).collect();
    Ok(actual)
}

/// Normalize only explicitly recorded native diagnostic categories.
fn check(row: &Row, mut actual: Value) -> TestResult {
    if let Some(kind) = row.expected["diagnosticKind"].as_str() {
        let message = actual["result"]["errorMessage"]
            .as_str()
            .ok_or("missing native diagnostic")?;
        match kind {
            "url" => assert!(message.starts_with("URL:")),
            "json" => assert!(message.starts_with("key must be a string")),
            "transport" => assert!(
                message == "Request was aborted" || message == "controlled selected cancellation"
            ),
            _ => panic!("unknown diagnostic category"),
        }
        actual["result"]
            .as_object_mut()
            .unwrap()
            .shift_remove("errorMessage");
        actual["diagnosticKind"] = json!(kind);
    }
    // Numeric spellings share the product JSON writer, while body bytes stay literal.
    let actual: Value = serde_json::from_str(&maestro_models::compact_json(&actual)?)?;
    let expected: Value = serde_json::from_str(&maestro_models::compact_json(&row.expected)?)?;
    for (actual, expected) in actual["requests"]
        .as_array()
        .unwrap()
        .iter()
        .zip(expected["requests"].as_array().unwrap())
    {
        assert_eq!(
            actual["headers"]
                .as_object()
                .unwrap()
                .keys()
                .collect::<Vec<_>>(),
            expected["headers"]
                .as_object()
                .unwrap()
                .keys()
                .collect::<Vec<_>>()
        );
    }
    assert!(
        actual == expected,
        "recorded output differs for {}: events={} result={}",
        row.test,
        actual["events"],
        actual["result"]
    );
    Ok(())
}

/// Execute each recorded row in a fresh single-test process, including server lifetime.
pub fn assert_rows(test: &str) -> TestResult {
    let rows: Vec<Row> =
        serde_json::from_str(include_str!("../fixtures/cloud_content_invocation.json"))?;
    let case = maestro_test_process::selection();
    if case.is_child() {
        let (_, row) = rows
            .iter()
            .enumerate()
            .find(|(index, _)| case.runs_case(&index.to_string()))
            .ok_or("unknown child case")?;
        return check_in_child(test, row);
    }
    let mut count = 0;
    for (index, row) in rows.iter().enumerate().filter(|(_, row)| row.test == test) {
        count += 1;
        let files = content_credentials::Files::new();
        let path = files.path("adc.json");
        let home = files.path("home");
        let mut env = vec![
            ("GOOGLE_APPLICATION_CREDENTIALS", path.as_str()),
            ("HOME", home.as_str()),
            ("NO_PROXY", "127.0.0.1,localhost"),
            ("METADATA_SERVER_DETECTION", "none"),
        ];
        if let Some(entries) = &row.input.env {
            env.extend(
                entries
                    .iter()
                    .map(|(key, value)| (key.as_str(), value.as_str())),
            );
        }
        {
            let output = maestro_test_process::TestProcess::current(test)?
                .nocapture()
                .env_clear()
                .envs(env.iter().copied())
                .case(index.to_string())
                .output()?;
            assert!(output.status.success(), "{output:?}");
        };
    }
    assert!(count > 0, "no recorded rows");
    Ok(())
}

/// Run the selected row with its own refresh server and credential files.
fn check_in_child(test: &str, row: &Row) -> TestResult {
    assert_eq!(test, row.test);
    let refreshes = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&refreshes);
    let server = loopback::serve_routed(move |request| {
        assert_eq!(request.request_line, "POST /refresh HTTP/1.1");
        assert!(String::from_utf8_lossy(&request.body).contains("grant_type=refresh_token"));
        count.fetch_add(1, Ordering::SeqCst);
        loopback::Reply {
            status: 200,
            content_type: "application/json",
            headers: &[],
            body: br#"{"access_token":"fixture-access","expires_in":3600}"#.to_vec(),
            close: false,
        }
    })?;
    let credentials =
        content_credentials::workforce(&format!("{}/refresh", server.url), "fixture-quota");
    if row.input.adc_appears_inside_hook != Some(true) {
        write_credentials(&credentials);
    }
    let result = crate::runtime::run(observe(&row.input, credentials, &refreshes));
    server.finish()?;
    check(row, result?)
}

/// Keep explicit null distinct from an omitted fixture option.
fn scope_value<'a>(value: &'a Presence<String>, default: Option<&'a str>) -> Option<&'a str> {
    match value {
        Presence::Missing => default,
        Presence::Null => None,
        Presence::Present(text) => Some(text),
    }
}

/// Select the requested public entry with the recorded raw or simple controls.
fn entry(
    input: &Input,
    common: StreamOptions,
) -> TestResult<Result<maestro_models::AssistantMessageEventStream, DiagnosticErrorInfo>> {
    let (model, context) = conversation(input)?;
    Ok(if input.simple == Some(true) {
        let options = Some(SimpleStreamOptions {
            common,
            reasoning: input.reasoning,
            thinking_budgets: input.thinking_budgets.clone(),
            ..SimpleStreamOptions::default()
        });
        if input.entry.as_deref() == Some("public") {
            maestro_models::providers::content::google::stream_simple_google(
                model, context, options,
            )
        } else {
            Ok(maestro_models::stream_simple_google_vertex(
                model, context, options,
            ))
        }
    } else {
        let scope = input.no_scope != Some(true);
        let extra = json!({"toolChoice":input.tool_choice,"thinking":input.thinking,
            "project":scope_value(&input.project, scope.then_some("fixture-project")),
            "location":scope_value(&input.location, scope.then_some("global"))});
        maestro_models::stream(
            model,
            context,
            Some(ProviderStreamOptions {
                common,
                extra: extra.as_object().unwrap().clone(),
                ..ProviderStreamOptions::default()
            }),
        )
    })
}

/// Controlled framed response from the independent recorder.
fn reply(failure: Option<&str>) -> HttpResponse {
    let (status, body) = match failure {
        Some("http") => (
            429,
            "{\"error\":{\"code\":429,\"message\":\"controlled status\"}}",
        ),
        Some("bad-json") => (200, "data: {bad}\n\n"),
        Some("body") => (
            200,
            "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"partial\"}]}}]}\n\n",
        ),
        _ => (
            200,
            "data: {\"responseId\":\"cloud-response\",\"candidates\":[{\"content\":{\"role\":\"model\",\"parts\":[{\"text\":\"ok\"}]},\"finishReason\":\"STOP\"}],\"usageMetadata\":{\"promptTokenCount\":5,\"cachedContentTokenCount\":1,\"candidatesTokenCount\":2,\"thoughtsTokenCount\":1,\"totalTokenCount\":8}}\n\n",
        ),
    };
    let mut chunks = vec![Ok(body.as_bytes().to_vec())];
    if failure == Some("body") {
        chunks.push(Err(FetchError::Connection(DiagnosticErrorInfo::error(
            "controlled body failure",
        ))));
    }
    HttpResponse {
        status,
        status_text: String::new(),
        headers: BTreeMap::from([(
            "content-type".into(),
            if status == 429 {
                "application/json"
            } else {
                "text/event-stream"
            }
            .into(),
        )]),
        body: Box::pin(futures_util::stream::iter(chunks)),
    }
}
