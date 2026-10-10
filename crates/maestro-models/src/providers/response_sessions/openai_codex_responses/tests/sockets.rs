//! Response-session socket selection, wire body and release witnesses over scripted sockets.

use super::super::websocket::{
    Receiver, WebSocketOutput, close_error, message_text, resolve_codex_web_socket_url,
    run_released, wire_body,
};
use super::super::{CodexError, headers::build_web_socket_headers};
use crate::providers::nullable::Nullable;
use crate::providers::responses::openai_responses::OpenAIResponsesServiceTier;
use crate::{
    AssistantMessageEvent, AssistantMessageEventStream, Cancellation, DiagnosticCode,
    SharedAssistantMessage,
};
use futures_core::Stream;
use futures_util::FutureExt;
use indexmap::IndexMap;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::{Arc, Mutex, atomic::AtomicUsize, atomic::Ordering};
use std::task::{Context, Poll};
use tokio_tungstenite::tungstenite::{
    Error, Message,
    protocol::{CloseFrame, frame::coding::CloseCode},
};

/// Drive a future to completion on a paused-clock runtime.
pub(super) fn block_on<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .start_paused(true)
        .build()
        .unwrap()
        .block_on(future)
}

/// One scripted receive step.
pub(super) enum Incoming {
    /// A complete message.
    Message(Message),
    /// A native receive failure with this text.
    Error(String),
    /// Cancel the operation instead of delivering anything.
    Abort,
}

/// A socket that replays scripted receives and records everything sent.
pub(super) struct Scripted {
    /// Receives not yet delivered; an exhausted script ends the stream.
    incoming: VecDeque<Incoming>,
    /// Messages written to the socket, including a refused request write.
    pub(super) sent: Arc<Mutex<Vec<Message>>>,
    /// The write that fails.
    fault: Fault,
    /// Signal the abort step cancels.
    signal: Cancellation,
}

impl Scripted {
    /// Start a script that cancels `signal` at its abort step.
    pub(super) fn new(incoming: Vec<Incoming>, signal: &Cancellation) -> Self {
        Self {
            incoming: incoming.into(),
            sent: Arc::default(),
            fault: Fault::None,
            signal: signal.clone(),
        }
    }
}

impl Stream for Scripted {
    type Item = Result<Message, Error>;
    fn poll_next(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.incoming.pop_front() {
            Some(Incoming::Message(message)) => Poll::Ready(Some(Ok(message))),
            Some(Incoming::Error(text)) => {
                Poll::Ready(Some(Err(Error::Io(std::io::Error::other(text)))))
            }
            Some(Incoming::Abort) => {
                self.signal.abort();
                Poll::Pending
            }
            None => Poll::Ready(None),
        }
    }
}

impl futures_util::Sink<Message> for Scripted {
    type Error = Error;
    fn poll_ready(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Error>> {
        Poll::Ready(Ok(()))
    }
    fn start_send(self: Pin<&mut Self>, item: Message) -> Result<(), Error> {
        let closing = matches!(item, Message::Close(_));
        if self.fault
            == if closing {
                Fault::Close
            } else {
                Fault::Request
            }
        {
            if !closing {
                self.sent.lock().unwrap().push(item);
            }
            return Err(Error::Io(std::io::Error::other("send failed")));
        }
        self.sent.lock().unwrap().push(item);
        Ok(())
    }
    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Error>> {
        Poll::Ready(Ok(()))
    }
    fn poll_close(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<(), Error>> {
        Poll::Ready(Ok(()))
    }
}

/// A scripted packet as a fixture authors it.
#[derive(Deserialize)]
#[serde(rename_all = "lowercase", deny_unknown_fields)]
enum Packet {
    /// Text message.
    Text(String),
    /// Binary message.
    Bytes(Vec<u8>),
    /// Close frame.
    Close(CloseCase),
    /// Native receive failure.
    Error(String),
    /// Cancellation.
    Abort(bool),
}

impl Packet {
    /// The scripted receive this packet describes.
    fn incoming(&self) -> Incoming {
        match self {
            Self::Text(text) => Incoming::Message(Message::text(text.clone())),
            Self::Bytes(bytes) => Incoming::Message(Message::binary(bytes.clone())),
            Self::Close(close) => Incoming::Message(close.message()),
            Self::Error(text) => Incoming::Error(text.clone()),
            Self::Abort(armed) => {
                assert!(armed, "abort packets are armed");
                Incoming::Abort
            }
        }
    }
}

/// A close code with its optional reason.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CloseCase {
    /// Numeric status.
    code: u16,
    /// Optional reason text.
    reason: Option<String>,
}

impl CloseCase {
    /// The close message carrying this status.
    fn message(&self) -> Message {
        Message::Close(Some(CloseFrame {
            code: CloseCode::from(self.code),
            reason: self.reason.clone().unwrap_or_default().into(),
        }))
    }
}

/// A failure's authored identity; a message ending in a native-cause marker is a prefix.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Failure {
    /// Error name.
    name: String,
    /// Complete message, or its fixed prefix.
    message: String,
    /// Optional error code.
    code: Option<DiagnosticCode>,
}

impl Failure {
    /// Assert the category implied by the name, the message and the code.
    fn assert_matches(&self, error: &CodexError, label: &str) {
        let (kind, info) = match error {
            CodexError::Api(info) => ("CodexApiError", info),
            CodexError::Protocol(info) => ("CodexProtocolError", info),
            CodexError::Transport(info) => ("transport", info),
        };
        if kind != "transport" {
            assert_eq!(kind, self.name, "{label}");
        }
        assert_eq!(info.name.as_deref(), Some(self.name.as_str()), "{label}");
        match self.message.strip_suffix("<native cause>") {
            Some(prefix) => assert!(info.message.starts_with(prefix), "{label}: {info:?}"),
            None => assert_eq!(info.message, self.message, "{label}"),
        }
        assert_eq!(info.code, self.code, "{label}");
    }
}

/// Everything one scripted run produced.
pub(super) struct Run {
    /// Operation outcome.
    pub(super) result: Result<(), CodexError>,
    /// Start notifications.
    pub(super) starts: usize,
    /// The filled message.
    pub(super) output: SharedAssistantMessage,
    /// Partial output carried by the start event, when one was produced.
    pub(super) start: Option<SharedAssistantMessage>,
    /// Producer events in order, by type name.
    pub(super) events: Vec<String>,
    /// Messages written to the socket, including a refused request write.
    pub(super) sent: Vec<Message>,
}

/// The write a scripted socket refuses.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Fault {
    /// Every write succeeds.
    None,
    /// The request write fails.
    Request,
    /// The closing write fails.
    Close,
}

/// Run one request over a scripted socket with the priority tier requested.
pub(super) async fn run_script(incoming: Vec<Incoming>, fault: Fault) -> Run {
    let (model, _, mut options) = super::invocation();
    options.service_tier = Some(Nullable::Value(OpenAIResponsesServiceTier::Priority));
    let signal = Cancellation::new();
    options.common.signal = Some(signal.clone());
    let mut socket = Scripted::new(incoming, &signal);
    socket.fault = fault;
    let sent = Arc::clone(&socket.sent);
    let output = Arc::new(std::sync::RwLock::new(
        crate::providers::assistant_output::initial_message(&model),
    ));
    output.write().unwrap().timestamp = 1.0;
    let events = AssistantMessageEventStream::new();
    let starts = AtomicUsize::new(0);
    let mut on_start = || {
        starts.fetch_add(1, Ordering::SeqCst);
    };
    let body =
        wire_body(&json!({"model":"m","store":false,"input":[{"role":"user","content":"hello"}]}))
            .unwrap();
    let result = run_released(
        socket,
        body,
        &model,
        &options,
        WebSocketOutput {
            output: &output,
            stream: &events,
            on_start: &mut on_start,
        },
    )
    .await;
    let mut names = Vec::new();
    let mut start = None;
    while let Some(Some(event)) = events.next().now_or_never() {
        names.push(
            serde_json::to_value(&event).unwrap()["type"]
                .as_str()
                .unwrap()
                .to_owned(),
        );
        if let AssistantMessageEvent::Start { partial } = event {
            start = Some(partial);
        }
    }
    let sent = sent.lock().unwrap().clone();
    Run {
        result,
        starts: starts.load(Ordering::SeqCst),
        output,
        start,
        events: names,
        sent,
    }
}

/// One authored endpoint and the socket endpoint it resolves to.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EndpointCase {
    /// Authored base URL.
    base: String,
    /// Resolved endpoint, or none when the URL is invalid.
    expected: Option<String>,
}

#[test]
fn maestro_response_sessions_resolve_socket_endpoint() {
    let rows: Vec<EndpointCase> =
        super::fixture_rows(include_str!("fixtures/socket_endpoints.json"), &["base"]).unwrap();
    for row in rows {
        match (resolve_codex_web_socket_url(&row.base), &row.expected) {
            (Ok(url), Some(expected)) => assert_eq!(&url, expected, "{:?}", row.base),
            (Err(CodexError::Transport(_)), None) => {}
            (other, _) => panic!("{:?}: {other:?}", row.base),
        }
    }
}

/// Header layers, the request identifier and the headers the socket must carry.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HeaderCase {
    /// Descriptor headers, combined by name.
    init: IndexMap<String, String>,
    /// Caller headers, replacing by name.
    additional: IndexMap<String, String>,
    /// Identifier for both affinity headers.
    request_id: String,
    /// Effective headers, or none when a layer or the identifier is invalid.
    expected: Option<IndexMap<String, String>>,
}

#[test]
fn maestro_response_sessions_apply_socket_headers() {
    let rows: Vec<HeaderCase> = super::fixture_rows(
        include_str!("fixtures/socket_headers.json"),
        &["init", "additional", "request_id"],
    )
    .unwrap();
    for row in rows {
        let mut model = super::controlled_model();
        model.headers = Some(row.init.clone());
        let options = crate::StreamOptions {
            headers: Some(row.additional.clone()),
            ..Default::default()
        };
        let derived = super::super::headers::build_sse_headers(
            &model,
            &options,
            "account",
            "fixture-token",
            "maestro (fixture)",
        )
        .and_then(|prepared| build_web_socket_headers(&prepared, &row.request_id));
        match (derived, row.expected) {
            (Ok(headers), Some(expected)) => assert_eq!(headers, expected, "{:?}", row.init),
            (Err(_), None) => {}
            (other, _) => panic!("{:?}: {other:?}", row.init),
        }
    }
}

/// A prepared document and the exact request text sent for it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BodyCase {
    /// Prepared body.
    body: Value,
    /// Compact request text.
    expected: String,
}

#[test]
fn maestro_response_sessions_write_negative_zero_as_zero() {
    let body = json!({"model":"m","temperature":-0.0,"input":[]});
    assert!(body["temperature"].as_f64().unwrap().is_sign_negative());
    assert_eq!(
        wire_body(&body).unwrap(),
        r#"{"type":"response.create","model":"m","temperature":0,"input":[]}"#
    );
}

#[test]
fn maestro_response_sessions_send_full_socket_body() {
    let text = include_str!("fixtures/socket_bodies.json");
    let rows: Vec<BodyCase> = super::fixture_rows(text, &["body"]).unwrap();
    for row in rows {
        assert_eq!(wire_body(&row.body).unwrap(), row.expected);
    }
}

/// Message payload and the text the provider selection reads from it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DataCase {
    /// Binary payload.
    bytes: Option<Vec<u8>>,
    /// Text payload.
    text: Option<String>,
    /// Decoded text.
    expected: String,
}

#[test]
fn maestro_response_sessions_decode_socket_data() {
    let rows: Vec<DataCase> = super::fixture_rows(
        include_str!("fixtures/socket_data.json"),
        &["bytes", "text"],
    )
    .unwrap();
    for row in rows {
        let message = match (&row.bytes, &row.text) {
            (Some(bytes), None) => Message::binary(bytes.clone()),
            (None, Some(text)) => Message::text(text.clone()),
            _ => panic!("one payload per row"),
        };
        assert_eq!(message_text(&message).unwrap(), row.expected);
    }
}

/// A scripted receive sequence and the selected events it yields.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EventCase {
    /// Scenario name for failure messages.
    scenario: String,
    /// Receives in order.
    packets: Vec<Packet>,
    /// Selected event texts, exactly as the reducer receives them, before the sequence ends.
    events: Vec<String>,
    /// Failure that ends it.
    error: Option<Failure>,
}

#[test]
fn maestro_response_sessions_select_socket_events() {
    let rows: Vec<EventCase> =
        super::fixture_rows(include_str!("fixtures/socket_events.json"), &["scenario"]).unwrap();
    block_on(async {
        for row in rows {
            let signal = Cancellation::new();
            let incoming = row.packets.iter().map(Packet::incoming).collect();
            let mut socket = Scripted::new(incoming, &signal);
            let (model, _, _) = super::invocation();
            let output = Arc::new(std::sync::RwLock::new(
                crate::providers::assistant_output::initial_message(&model),
            ));
            let events = AssistantMessageEventStream::new();
            let mut on_start = || {};
            let mut receiver = Receiver::new(
                &mut socket,
                Some(&signal),
                WebSocketOutput {
                    output: &output,
                    stream: &events,
                    on_start: &mut on_start,
                },
            );
            let mut selected = Vec::new();
            while let Some(Ok(text)) = receiver.next().await {
                selected.push(text);
            }
            assert_eq!(selected, row.events, "{}", row.scenario);
            match (&row.error, receiver.failure.take()) {
                (Some(expected), Some(error)) => expected.assert_matches(&error, &row.scenario),
                (None, None) => {}
                (expected, actual) => {
                    panic!("{}: {:?} vs {actual:?}", row.scenario, expected.is_some())
                }
            }
        }
    });
}

/// A close status and the diagnostic it becomes.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CloseExpectation {
    /// Numeric status.
    code: u16,
    /// Optional reason.
    reason: Option<String>,
    /// Authored diagnostic.
    expected: Failure,
}

#[test]
fn maestro_response_sessions_distinguish_socket_failures() {
    let rows: Vec<CloseExpectation> = super::fixture_rows(
        include_str!("fixtures/socket_failures.json"),
        &["code", "reason"],
    )
    .unwrap();
    for row in rows {
        let error = close_error(row.code, row.reason.as_deref().unwrap_or_default());
        row.expected
            .assert_matches(&error, &format!("{}", row.code));
    }
}

/// A scripted whole operation and everything it must produce.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LifecycleCase {
    /// Scenario name for failure messages.
    scenario: String,
    /// Receives in order.
    packets: Vec<Packet>,
    /// The request write fails.
    send_fails: bool,
    /// Text of each request write attempted.
    sent: Vec<String>,
    /// Closes written, in order.
    closes: Vec<CloseCase>,
    /// Start notifications.
    starts: usize,
    /// Producer event types in order.
    events: Vec<String>,
    /// The filled message.
    message: Value,
    /// Failure that ended the operation.
    error: Option<Failure>,
}

/// Read the compact spelling back, so integral doubles compare with integers.
fn numbers_as_written(value: &Value) -> Value {
    let text = crate::providers::json_text::compact_json(value).unwrap();
    serde_json::from_str(&text).unwrap()
}

/// The start event shares the operation's output, so it shows the finished message.
fn assert_start_handle(run: &Run, finished: &Value, label: &str) {
    match &run.start {
        Some(start) => {
            assert!(
                Arc::ptr_eq(start, &run.output),
                "{label}: start handle is a copy"
            );
            let retained = serde_json::to_value(start.read().unwrap().clone()).unwrap();
            assert_eq!(&retained, finished, "{label}");
        }
        None => assert!(!run.events.contains(&"start".to_owned()), "{label}"),
    }
}

#[test]
fn maestro_response_sessions_run_uncached_socket_lifecycle() {
    let rows: Vec<LifecycleCase> = super::fixture_rows(
        include_str!("fixtures/socket_lifecycle.json"),
        &["scenario"],
    )
    .unwrap();
    block_on(async {
        for row in rows {
            let incoming = row.packets.iter().map(Packet::incoming).collect();
            let fault = if row.send_fails {
                Fault::Request
            } else {
                Fault::None
            };
            let run = run_script(incoming, fault).await;
            let texts: Vec<String> = run
                .sent
                .iter()
                .filter_map(|message| match message {
                    Message::Text(text) => Some(text.as_str().to_owned()),
                    _ => None,
                })
                .collect();
            let closes: Vec<(u16, String)> = run
                .sent
                .iter()
                .filter_map(|message| match message {
                    Message::Close(Some(frame)) => {
                        Some((frame.code.into(), frame.reason.as_str().to_owned()))
                    }
                    _ => None,
                })
                .collect();
            let expected: Vec<(u16, String)> = row
                .closes
                .iter()
                .map(|close| (close.code, close.reason.clone().unwrap_or_default()))
                .collect();
            let label = &row.scenario;
            assert_eq!(closes, expected, "{label}");
            assert_eq!(run.starts, row.starts, "{label}");
            assert_eq!(run.events, row.events, "{label}");
            assert_eq!(texts, row.sent, "{label}");
            let snapshot = run.output.read().unwrap().clone();
            let actual = serde_json::to_value(snapshot).unwrap();
            assert_start_handle(&run, &actual, label);
            assert_eq!(
                numbers_as_written(&actual),
                numbers_as_written(&row.message),
                "{label}"
            );
            match (&row.error, &run.result) {
                (Some(expected), Err(error)) => expected.assert_matches(error, label),
                (None, Ok(())) => {}
                (_, outcome) => panic!("{label}: {outcome:?}"),
            }
        }
    });
}

/// Select every event a socket yields for these receives and the failure that ends them.
async fn select_all(incoming: Vec<Incoming>) -> (Vec<String>, Option<CodexError>) {
    let signal = Cancellation::new();
    let mut socket = Scripted::new(incoming, &signal);
    let (model, _, _) = super::invocation();
    let output = Arc::new(std::sync::RwLock::new(
        crate::providers::assistant_output::initial_message(&model),
    ));
    let events = AssistantMessageEventStream::new();
    let mut on_start = || {};
    let mut receiver = Receiver::new(
        &mut socket,
        Some(&signal),
        WebSocketOutput {
            output: &output,
            stream: &events,
            on_start: &mut on_start,
        },
    );
    let mut selected = Vec::new();
    while let Some(Ok(text)) = receiver.next().await {
        selected.push(text);
    }
    (selected, receiver.failure.take())
}

#[test]
fn maestro_response_sessions_keep_sse_selection_separate() {
    use super::super::events::Source;
    use futures_util::stream;
    let event = r#"{"type":"future.event","n":1}"#;
    let completed = r#"{"type":"response.completed","response":{"id":"r","status":"completed"}}"#;
    let bom = '\u{feff}';
    let sse = format!("data: [DONE]\n\ndata: {bom}{event}\n\ndata: {completed}\n\n");
    block_on(async {
        let mut source = Source::new(Box::pin(stream::iter([Ok(sse.into_bytes())])), None);
        let mut selected = Vec::new();
        while let Some(Ok(text)) = source.next().await {
            selected.push(text);
        }
        assert_eq!(selected.len(), 2);
        assert_eq!(selected[0], event);
        for text in ["[DONE]", "\u{feff}{\"type\":\"future.event\"}"] {
            let (selected, failure) =
                select_all(vec![Incoming::Message(Message::text(text))]).await;
            assert!(selected.is_empty(), "{text}");
            assert!(matches!(failure, Some(CodexError::Protocol(_))), "{text}");
        }
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend(event.as_bytes());
        let (selected, failure) = select_all(vec![Incoming::Message(Message::binary(bytes))]).await;
        assert_eq!(selected, [event]);
        assert!(matches!(failure, Some(CodexError::Transport(_))));
    });
}

/// Receives that fail after some output and the category, start count and kept text they leave.
struct PhaseCase {
    /// Scenario name for failure messages.
    scenario: &'static str,
    /// Receives in order.
    incoming: Vec<Incoming>,
    /// Start notifications.
    starts: usize,
    /// Visible text kept in the message.
    kept: &'static str,
    /// Expected category, name and message prefix.
    category: &'static str,
    /// Expected name.
    name: Option<&'static str>,
    /// Expected message prefix.
    message: &'static str,
}

/// The text events that precede a failure.
fn partial_text() -> Vec<Incoming> {
    [
        json!({"type":"response.created","response":{"id":"r1"}}),
        json!({"type":"response.output_item.added","item":{"type":"message","id":"m1","role":"assistant","status":"in_progress","content":[]}}),
        json!({"type":"response.content_part.added","part":{"type":"output_text","text":""}}),
        json!({"type":"response.output_text.delta","delta":"kept"}),
    ]
    .iter()
    .map(|event| Incoming::Message(Message::text(event.to_string())))
    .collect()
}

/// Receives that fail after some output, in several categories.
fn phase_cases() -> [PhaseCase; 5] {
    let mut json_after = partial_text();
    json_after.push(Incoming::Message(Message::text("{bad}")));
    let mut closed_after = partial_text();
    closed_after.push(Incoming::Message(Message::Close(None)));
    let mut vanished_after = partial_text();
    vanished_after.push(Incoming::Error("reset".to_owned()));
    let mut failed_after = partial_text();
    failed_after.push(Incoming::Message(Message::text(
        r#"{"type":"response.created"}"#,
    )));
    [
        PhaseCase {
            scenario: "api before start",
            incoming: vec![Incoming::Message(Message::text(
                r#"{"type":"error","message":"no"}"#,
            ))],
            starts: 0,
            kept: "",
            category: "api",
            name: Some("CodexApiError"),
            message: "Codex error: no",
        },
        PhaseCase {
            scenario: "json after output",
            incoming: json_after,
            starts: 1,
            kept: "kept",
            category: "protocol",
            name: Some("CodexProtocolError"),
            message: "Invalid Codex WebSocket JSON: ",
        },
        PhaseCase {
            scenario: "close without status after output",
            incoming: closed_after,
            starts: 1,
            kept: "kept",
            category: "transport",
            name: Some("WebSocketCloseError"),
            message: "WebSocket closed 1005",
        },
        PhaseCase {
            scenario: "native failure after output",
            incoming: vanished_after,
            starts: 1,
            kept: "kept",
            category: "transport",
            name: Some("Error"),
            message: "IO error: reset",
        },
        PhaseCase {
            scenario: "reducer failure after output",
            incoming: failed_after,
            starts: 1,
            kept: "kept",
            category: "protocol",
            name: None,
            message: "missing or null required container",
        },
    ]
}

#[test]
fn maestro_response_sessions_preserve_socket_error_phase() {
    let cases = phase_cases();
    block_on(async {
        for case in cases {
            let run = run_script(case.incoming, Fault::None).await;
            let label = case.scenario;
            let (category, info) = match run.result.as_ref().unwrap_err() {
                CodexError::Api(info) => ("api", info),
                CodexError::Protocol(info) => ("protocol", info),
                CodexError::Transport(info) => ("transport", info),
            };
            assert_eq!(category, case.category, "{label}");
            assert_eq!(info.name.as_deref(), case.name, "{label}");
            assert!(
                info.message.starts_with(case.message),
                "{label}: {}",
                info.message
            );
            assert_eq!(run.starts, case.starts, "{label}");
            let message = serde_json::to_value(run.output.read().unwrap().clone()).unwrap();
            let kept = message["content"][0]["text"].as_str().unwrap_or_default();
            assert_eq!(kept, case.kept, "{label}");
        }
    });
}
