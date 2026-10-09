//! Request construction and sending for direct message-protocol invocations.

#[allow(dead_code, reason = "Each test binary uses part of the chat fixtures.")]
#[path = "support/chat.rs"]
mod chat;
#[path = "support/child_process.rs"]
mod child_process;
#[path = "support/json.rs"]
mod json;
#[allow(
    dead_code,
    reason = "Each test binary uses part of the loopback server."
)]
#[path = "support/loopback.rs"]
mod loopback;
#[allow(
    dead_code,
    reason = "Each test binary uses part of the message fixtures."
)]
#[path = "support/messages.rs"]
mod messages;
#[allow(
    dead_code,
    reason = "Each test binary uses part of the scripted transport."
)]
#[path = "support/transport.rs"]
mod transport;

use std::sync::{Arc, Mutex, OnceLock};

use chat::{TestResult, block_on};
use child_process::child_case;
use futures_util::FutureExt;
use maestro_models::{
    AnthropicClient, AnthropicOptions, AnthropicRequestOptions, AssistantMessageEvent,
    AssistantMessageEventStream, Cancellation, HttpResponse, Model, OnPayload, OnResponse,
    stream_anthropic,
};
use serde_json::{Value, json};
use transport::{Attempt, transport};

const FIXTURE: &str = include_str!("fixtures/message_protocol/requests.json");

/// A message start followed by the stop that ends the stream.
fn short_answer() -> Vec<u8> {
    let start = r#"{"type":"message_start","message":{"id":"msg_fixture","usage":{"input_tokens":12,"output_tokens":1}}}"#;
    format!("event: message_start\ndata: {start}\n\n{}", messages::STOP).into_bytes()
}

/// The default client posts the converted request to the model's endpoint.
async fn default_client_posts_request() -> TestResult {
    let server = loopback::serve(
        loopback::EVENT_STREAM_HEAD,
        vec![short_answer()],
        std::time::Duration::ZERO,
    )?;
    let patch = json!({"baseUrl": server.url});
    let options = messages::options(&messages::Case::default())?;
    let stream = stream_anthropic(
        messages::model(&patch)?,
        messages::context(&Value::Null)?,
        Some(options),
    );
    let (_, result) = messages::collect(&stream).await?;
    assert_eq!(result["stopReason"], "stop");
    assert_eq!(result["responseId"], "msg_fixture");
    let received = server.finish()?;
    assert_eq!(received.request_line, "POST /v1/messages HTTP/1.1");
    for (name, value) in [
        ("x-api-key", "fixture-key"),
        ("content-type", "application/json"),
        ("anthropic-version", "2023-06-01"),
        ("accept", "application/json"),
        ("anthropic-dangerous-direct-browser-access", "true"),
    ] {
        assert_eq!(received.header(name), Some(value), "{name}");
    }
    let body: Value = serde_json::from_slice(&received.body)?;
    assert_eq!(body["model"], "claude-sonnet-4-5");
    assert_eq!(body["stream"], true);
    Ok(())
}

#[test]
fn messages_send_key_request() -> TestResult {
    block_on(false, async {
        messages::assert_rows(FIXTURE, "messages_send_key_request").await?;
        if child_case().as_deref() == Some("*") {
            default_client_posts_request().await?;
        }
        Ok(())
    })
}

/// Numbers JSON cannot spell are written as `null`, an output limit that is not a number falls
/// back to the default, and negative zero is written as `0`.
async fn nonfinite_numbers_are_written_as_json_does() -> TestResult {
    let cases = [
        (
            f64::NAN,
            r#""max_tokens":10666,"stream":true,"temperature":null"#,
        ),
        (
            f64::INFINITY,
            r#""max_tokens":null,"stream":true,"temperature":null"#,
        ),
        (
            f64::NEG_INFINITY,
            r#""max_tokens":null,"stream":true,"temperature":null"#,
        ),
        (-0.0, r#""max_tokens":10666,"stream":true,"temperature":0"#),
    ];
    for (number, tail) in cases {
        let requests = messages::Requests::default();
        let mut options = messages::options(&messages::Case::default())?;
        options.common.max_tokens = Some(number);
        options.common.temperature = Some(number);
        options.common.fetch = Some(messages::scripted_fetch(
            &messages::Case::default(),
            &requests,
        ));
        let empty = json!({"messages": []});
        let stream = stream_anthropic(
            messages::model(&json!({}))?,
            messages::context(&empty)?,
            Some(options),
        );
        messages::collect(&stream).await?;
        let sent = requests.lock().map_err(|error| error.to_string())?.clone();
        let expected = format!(r#"{{"model":"claude-sonnet-4-5","messages":[],{tail}}}"#);
        assert_eq!(sent[0]["text"], json!(expected), "{number}");
    }
    Ok(())
}

#[test]
fn messages_apply_raw_token_default() -> TestResult {
    block_on(false, async {
        messages::assert_rows(FIXTURE, "messages_apply_raw_token_default").await?;
        if child_case().as_deref() == Some("*") {
            nonfinite_numbers_are_written_as_json_does().await?;
        }
        Ok(())
    })
}

#[test]
fn messages_filter_blank_content() -> TestResult {
    block_on(
        false,
        messages::assert_rows(FIXTURE, "messages_filter_blank_content"),
    )
}

#[test]
fn messages_replay_thinking() -> TestResult {
    block_on(
        false,
        messages::assert_rows(FIXTURE, "messages_replay_thinking"),
    )
}

#[test]
fn messages_project_foreign_replay() -> TestResult {
    block_on(
        false,
        messages::assert_rows(FIXTURE, "messages_project_foreign_replay"),
    )
}

#[test]
fn messages_normalize_tool_ids() -> TestResult {
    block_on(
        false,
        messages::assert_rows(FIXTURE, "messages_normalize_tool_ids"),
    )
}

#[test]
fn messages_group_tool_results() -> TestResult {
    block_on(
        false,
        messages::assert_rows(FIXTURE, "messages_group_tool_results"),
    )
}

#[test]
fn messages_place_default_cache() -> TestResult {
    block_on(
        false,
        messages::assert_rows(FIXTURE, "messages_place_default_cache"),
    )
}

#[test]
fn messages_select_tool_input_mode() -> TestResult {
    block_on(
        false,
        messages::assert_rows(FIXTURE, "messages_select_tool_input_mode"),
    )
}

#[test]
fn messages_project_tool_schemas() -> TestResult {
    block_on(
        false,
        messages::assert_rows(FIXTURE, "messages_project_tool_schemas"),
    )
}

/// The payload members follow one order in the body text.
async fn payload_members_are_written_in_order() -> TestResult {
    let case = messages::Case {
        context: json!({
            "systemPrompt": "system",
            "messages": [{"role": "user", "content": "ask"}],
            "tools": [{"name": "lookup", "description": "Find", "parameters": {}}]
        }),
        options: json!({"temperature": 0, "metadata": {"user_id": "id"}, "toolChoice": "auto"}),
        ..messages::Case::default()
    };
    let run = messages::run_case(&case).await?;
    let text = run.requests[0]["text"].as_str().ok_or("body text")?;
    let body: serde_json::Map<String, Value> = serde_json::from_str(text)?;
    let order: Vec<&str> = body.keys().map(String::as_str).collect();
    assert_eq!(
        order,
        [
            "model",
            "messages",
            "max_tokens",
            "stream",
            "system",
            "temperature",
            "tools",
            "metadata",
            "tool_choice"
        ]
    );
    Ok(())
}

#[test]
fn messages_apply_metadata_and_tool_choice() -> TestResult {
    block_on(false, async {
        messages::assert_rows(FIXTURE, "messages_apply_metadata_and_tool_choice").await?;
        if child_case().as_deref() == Some("*") {
            payload_members_are_written_in_order().await?;
        }
        Ok(())
    })
}

#[test]
fn messages_resolve_cache_retention() -> TestResult {
    block_on(
        false,
        messages::assert_rows(FIXTURE, "messages_resolve_cache_retention"),
    )
}

#[test]
fn messages_merge_protocol_headers() -> TestResult {
    block_on(
        false,
        messages::assert_rows(FIXTURE, "messages_merge_protocol_headers"),
    )
}

#[test]
fn messages_keep_http_error_envelopes() -> TestResult {
    block_on(
        false,
        messages::assert_rows(FIXTURE, "messages_keep_http_error_envelopes"),
    )
}

/// Both selected error-detail branches leave empty text when conversion exceeds its bound.
#[test]
fn messages_keep_empty_error_detail_at_conversion_boundary() -> TestResult {
    block_on(false, async {
        let nested = format!("{}0{}", "[".repeat(128), "]".repeat(128));
        for body in [format!(r#"{{"message":{nested}}}"#), nested] {
            let case = messages::Case {
                status: Some(400),
                chunks: Some(vec![messages::Chunk::Text(body)]),
                ..messages::Case::default()
            };
            let run = messages::run_case(&case).await?;
            assert_eq!(run.result["errorMessage"], json!("400 "));
            assert_eq!(run.result["stopReason"], json!("error"));
        }
        Ok(())
    })
}

/// Called outside any Tokio runtime, the call still returns its stream, which ends at once with
/// an error update.
fn call_outside_a_runtime_ends_in_error() -> TestResult {
    let stream = stream_anthropic(
        messages::model(&json!({}))?,
        messages::context(&Value::Null)?,
        Some(messages::options(&messages::Case::default())?),
    );
    let (events, result) = block_on(false, messages::collect(&stream))?;
    assert_eq!(events, [json!({"type": "error"})]);
    assert_eq!(
        result["errorMessage"],
        "Streaming requires a running Tokio runtime."
    );
    assert_eq!(result["stopReason"], "error");
    Ok(())
}

/// The payload hook runs before the authentication check that fails the call, and no request is
/// sent.
async fn payload_hook_precedes_authentication_failure() -> TestResult {
    let order = Order::default();
    let case = messages::Case {
        options: json!({"apiKey": ""}),
        ..messages::Case::default()
    };
    let requests = messages::Requests::default();
    let mut options = messages::options(&case)?;
    options.common.fetch = Some(messages::scripted_fetch(&case, &requests));
    options.common.on_payload = Some(payload_hook(&order, Payload::Keep));
    let stream = stream_anthropic(
        messages::model(&json!({}))?,
        messages::context(&Value::Null)?,
        Some(options),
    );
    let (_, result) = messages::collect(&stream).await?;
    let labels = order.labels.lock().map_err(|error| error.to_string())?;
    assert_eq!(*labels, ["payload"]);
    assert!(
        result["errorMessage"]
            .as_str()
            .is_some_and(|text| text.starts_with("Could not resolve authentication method")),
        "{result}"
    );
    assert!(
        requests
            .lock()
            .map_err(|error| error.to_string())?
            .is_empty()
    );
    Ok(())
}

#[test]
fn messages_keep_auth_failure_in_stream() -> TestResult {
    block_on(false, async {
        messages::assert_rows(FIXTURE, "messages_keep_auth_failure_in_stream").await?;
        if child_case().as_deref() == Some("*") {
            payload_hook_precedes_authentication_failure().await?;
        }
        Ok(())
    })?;
    if child_case().is_none() {
        call_outside_a_runtime_ends_in_error()?;
    }
    Ok(())
}

/// Labels of what happened, in order, and the call whose updates a callback can already see.
#[derive(Default)]
struct Steps {
    /// The labels so far.
    labels: Mutex<Vec<String>>,
    /// The stream of the call under test, set before the call can run.
    stream: OnceLock<AssistantMessageEventStream>,
    /// The models the hooks received.
    models: Mutex<Vec<Arc<Model>>>,
}

/// The steps of one call, shared with its callbacks.
type Order = Arc<Steps>;

/// The kind of an update, as its serialized `type`.
fn kind_of(event: &AssistantMessageEvent) -> String {
    serde_json::to_value(event)
        .ok()
        .and_then(|value| value["type"].as_str().map(str::to_owned))
        .unwrap_or_default()
}

impl Steps {
    /// Record a step, preceded by the kind of every update the call had announced by then.
    fn note(&self, label: impl Into<String>) {
        let announced: Vec<String> = self
            .stream
            .get()
            .into_iter()
            .flat_map(|stream| std::iter::from_fn(move || stream.next().now_or_never().flatten()))
            .map(|event| kind_of(&event))
            .collect();
        if let Ok(mut labels) = self.labels.lock() {
            labels.extend(announced);
            labels.push(label.into());
        }
    }
}

/// What a hook does with the payload it receives.
#[derive(Clone)]
enum Payload {
    /// Return it as it is.
    Keep,
    /// Return it with a temperature added.
    Edit,
    /// Return another value.
    Replace,
    /// Return a value that is not an object.
    Scalar,
    /// Fail with this message.
    Fail(&'static str),
}

/// How one call with an injected client went.
struct Hooked {
    /// Hook, client and update labels in the order they happened.
    order: Vec<String>,
    /// The models the payload and response hooks received.
    models: Vec<Arc<Model>>,
    /// The payload the client received, if it was called.
    sent: Option<Value>,
    /// The final message without its timestamp.
    result: Value,
}

/// An injected client that notes its call, keeps the payload and answers with status 201.
fn answering_client(order: &Order, sent: &Arc<Mutex<Option<Value>>>) -> AnthropicClient {
    let (order, sent) = (Arc::clone(order), Arc::clone(sent));
    Arc::new(move |payload, _| {
        order.note("client");
        if let Ok(mut kept) = sent.lock() {
            *kept = Some(payload);
        }
        Box::pin(std::future::ready(Ok(HttpResponse {
            status_text: String::new(),
            status: 201,
            headers: [("x-custom".to_owned(), "yes".to_owned())].into(),
            body: Box::pin(futures_util::stream::iter([Ok(short_answer())])),
        })))
    })
}

/// Keep the model a hook received.
fn keep_model(order: &Order, model: Arc<Model>) {
    if let Ok(mut models) = order.models.lock() {
        models.push(model);
    }
}

/// A payload hook that notes its call and applies `action`.
fn payload_hook(order: &Order, action: Payload) -> OnPayload {
    let order = Arc::clone(order);
    Arc::new(move |mut payload, model| {
        order.note("payload");
        keep_model(&order, model);
        let outcome = match &action {
            Payload::Keep => Ok(payload),
            Payload::Edit => {
                payload["temperature"] = json!(0.25);
                Ok(payload)
            }
            Payload::Replace => Ok(json!({"model": "replaced", "stream": false, "temperature": 9})),
            Payload::Scalar => Ok(json!(7)),
            Payload::Fail(message) => Err(messages::diagnostic(message)),
        };
        Box::pin(std::future::ready(outcome))
    })
}

/// A response hook that notes the status and a header it saw and may fail.
fn response_hook(order: &Order, failure: Option<&'static str>) -> OnResponse {
    let order = Arc::clone(order);
    Arc::new(move |response, model| {
        keep_model(&order, model);
        let custom = response
            .headers
            .get("x-custom")
            .cloned()
            .unwrap_or_default();
        order.note(format!("onResponse:{}:{custom}", response.status));
        Box::pin(std::future::ready(match failure {
            Some(message) => Err(messages::diagnostic(message)),
            None => Ok(()),
        }))
    })
}

/// Make one call through an injected client with the given hooks.
async fn hooked_call(
    payload: Payload,
    response_failure: Option<&'static str>,
) -> TestResult<Hooked> {
    let order = Order::default();
    let sent = Arc::new(Mutex::new(None));
    let mut options = AnthropicOptions {
        client: Some(answering_client(&order, &sent)),
        ..AnthropicOptions::default()
    };
    options.common.on_payload = Some(payload_hook(&order, payload));
    options.common.on_response = Some(response_hook(&order, response_failure));
    let model = messages::model(&json!({"baseUrl": "not a URL"}))?;
    let context = messages::context(&json!({"messages": []}))?;
    let stream = stream_anthropic(model, context, Some(options));
    order
        .stream
        .set(stream.clone())
        .map_err(|_| "the call is observed once")?;
    stream.result().await;
    let (events, result) = messages::collect(&stream).await?;
    for event in &events {
        order.note(event["type"].as_str().unwrap_or_default());
    }
    let models = order
        .models
        .lock()
        .map_err(|error| error.to_string())?
        .clone();
    let order = order
        .labels
        .lock()
        .map_err(|error| error.to_string())?
        .clone();
    let sent = sent.lock().map_err(|error| error.to_string())?.clone();
    Ok(Hooked {
        order,
        models,
        sent,
        result,
    })
}

/// The expected default payload with `members` added or replaced.
fn default_payload(members: &Value) -> Value {
    let mut payload =
        json!({"model": "claude-sonnet-4-5", "messages": [], "max_tokens": 10666, "stream": true});
    if let (Some(target), Some(extra)) = (payload.as_object_mut(), members.as_object()) {
        target.extend(extra.iter().map(|(k, v)| (k.clone(), v.clone())));
    }
    payload
}

#[test]
fn messages_keep_callback_order_and_replacement() -> TestResult {
    block_on(false, async {
        let steps = ["payload", "client", "onResponse:201:yes", "start", "done"];
        let cases = [
            (Payload::Keep, default_payload(&json!({}))),
            (
                Payload::Edit,
                default_payload(&json!({"temperature": 0.25})),
            ),
            (
                Payload::Replace,
                json!({"model": "replaced", "stream": true, "temperature": 9}),
            ),
            (Payload::Scalar, json!({"stream": true})),
        ];
        for (action, expected) in cases {
            let call = hooked_call(action, None).await?;
            assert_eq!(call.order, steps);
            assert!(
                matches!(&call.models[..], [first, second] if Arc::ptr_eq(first, second) && first.id == "claude-sonnet-4-5"),
                "both hooks receive the one model of the request"
            );
            assert_eq!(
                call.sent.map(json::canonical),
                Some(json::canonical(expected))
            );
            assert_eq!(call.result["stopReason"], "stop");
        }
        let refused = hooked_call(Payload::Fail("payload refused"), None).await?;
        assert_eq!(refused.order, ["payload", "error"]);
        assert!(
            refused.sent.is_none(),
            "the client is not called after a payload failure"
        );
        assert_eq!(refused.result["errorMessage"], "payload refused");
        let rejected = hooked_call(Payload::Keep, Some("response refused")).await?;
        assert_eq!(
            rejected.order,
            ["payload", "client", "onResponse:201:yes", "error"]
        );
        assert_eq!(rejected.result["errorMessage"], "response refused");
        for (payload, response) in [(Payload::Fail(""), None), (Payload::Keep, Some(""))] {
            let empty = hooked_call(payload, response).await?;
            assert_eq!(
                empty.result["errorMessage"], "",
                "an empty message stays empty"
            );
            assert_eq!(empty.result["stopReason"], "error");
        }
        Ok(())
    })
}

/// An injected client that keeps the settings it receives.
fn recording_client(
    received: &Arc<Mutex<Vec<(Value, AnthropicRequestOptions)>>>,
) -> AnthropicClient {
    let received = Arc::clone(received);
    Arc::new(move |payload, settings| {
        if let Ok(mut calls) = received.lock() {
            calls.push((payload, settings));
        }
        Box::pin(std::future::ready(Ok(HttpResponse {
            status_text: String::new(),
            status: 200,
            headers: std::collections::BTreeMap::new(),
            body: Box::pin(futures_util::stream::iter([Ok(messages::STOP
                .as_bytes()
                .to_vec())])),
        })))
    })
}

/// Call with an injected client, no key and an endpoint that is not a URL.
async fn injected_call(
    tune: impl FnOnce(&mut AnthropicOptions),
) -> TestResult<(Vec<(Value, AnthropicRequestOptions)>, Value)> {
    let received = Arc::default();
    let mut options = AnthropicOptions {
        client: Some(recording_client(&received)),
        ..AnthropicOptions::default()
    };
    tune(&mut options);
    let model = messages::model(&json!({"baseUrl": "not a URL"}))?;
    let stream = stream_anthropic(model, messages::context(&Value::Null)?, Some(options));
    let (_, result) = messages::collect(&stream).await?;
    let calls = received
        .lock()
        .map_err(|error| error.to_string())?
        .drain(..)
        .collect();
    Ok((calls, result))
}

/// An injected client needs neither the time nor the I/O driver of the runtime.
fn injected_call_runs_without_drivers() -> TestResult {
    tokio::runtime::Builder::new_current_thread()
        .build()?
        .block_on(async {
            let (_, result) = injected_call(|_| {}).await?;
            assert_eq!(result["stopReason"], "stop");
            Ok(())
        })
}

/// A client's response is read as an event stream whatever its status, and a failure it reports
/// ends the call with its message as it is.
async fn client_owns_status_and_failures() -> TestResult {
    for status in [201, 404, 500] {
        let (_, result) = injected_call(|options| {
            options.client = Some(messages::status_client(status));
        })
        .await?;
        assert_eq!(result["stopReason"], "stop", "status {status}");
    }
    for message in ["", "retained detail"] {
        let (_, result) = injected_call(|options| {
            options.client = Some(messages::failing_client(message));
        })
        .await?;
        assert_eq!(result["errorMessage"], message);
        assert_eq!(result["stopReason"], "error");
    }
    Ok(())
}

#[test]
fn messages_inject_client_without_auth() -> TestResult {
    injected_call_runs_without_drivers()?;
    block_on(false, async {
        let (calls, result) = injected_call(|_| {}).await?;
        assert_eq!(
            result["stopReason"], "stop",
            "no key and no endpoint are needed"
        );
        assert_eq!(calls.len(), 1);
        let (payload, settings) = &calls[0];
        assert_eq!(payload["stream"], true);
        assert_eq!(
            payload["messages"][0]["content"][0]["cache_control"],
            json!({"type": "ephemeral"}),
            "the payload follows the same cache rules"
        );
        assert!(
            settings.signal.is_none()
                && settings.timeout_ms.is_none()
                && settings.max_retries.is_none()
        );
        for value in [None, Some(0.0), Some(2.0)] {
            let (calls, _) = injected_call(|options| {
                options.common.timeout_ms = value;
                options.common.max_retries = value;
                options.common.signal = value.map(|_| Cancellation::new());
            })
            .await?;
            let settings = &calls[0].1;
            assert_eq!(settings.timeout_ms, value);
            assert_eq!(settings.max_retries, value);
            assert_eq!(settings.signal.is_some(), value.is_some());
        }
        client_owns_status_and_failures().await
    })
}

/// Make one call over the scripted transport and report the final stop reason and hook calls.
async fn retried_call(
    attempts: Vec<Attempt>,
    max_retries: f64,
) -> TestResult<(transport::Transport, Value, Vec<f64>)> {
    let target = transport(attempts);
    let statuses: Arc<Mutex<Vec<f64>>> = Arc::default();
    let seen = Arc::clone(&statuses);
    let mut options = messages::options(&messages::Case::default())?;
    options.common.max_retries = Some(max_retries);
    options.common.fetch = Some(Arc::clone(&target.fetch));
    options.common.on_response = Some(Arc::new(move |response, _| {
        if let Ok(mut statuses) = seen.lock() {
            statuses.push(response.status);
        }
        Box::pin(std::future::ready(Ok(())))
    }));
    let stream = stream_anthropic(
        messages::model(&json!({}))?,
        messages::context(&Value::Null)?,
        Some(options),
    );
    let (_, result) = messages::collect(&stream).await?;
    let statuses = statuses.lock().map_err(|error| error.to_string())?.clone();
    Ok((target, result, statuses))
}

#[test]
fn messages_reuse_setup_retry_policy() -> TestResult {
    block_on(true, async {
        let busy = || Attempt::body(500, &[], b"busy".to_vec());
        let answer = || Attempt::body(200, &[], short_answer());
        let (target, result, statuses) = retried_call(vec![busy(), answer()], 2.0).await?;
        assert_eq!(result["stopReason"], "stop");
        assert_eq!(target.attempts(), 2, "a transient status is retried");
        assert_eq!(
            statuses,
            [200.0],
            "the response hook sees only the accepted response"
        );

        let (target, result, statuses) = retried_call(vec![busy(), busy(), answer()], 1.0).await?;
        assert_eq!(
            target.attempts(),
            2,
            "the retry budget is the configured one"
        );
        assert_eq!(result["errorMessage"], "500 busy");
        assert!(statuses.is_empty(), "no response hook for a failed request");
        Ok(())
    })
}
