//! Public transport selection against controlled peers.

use super::super::stream_openai_codex_responses;
use crate::tests::runtime::run;
use crate::{StopReason, Transport};
use futures_util::{SinkExt, StreamExt};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tokio::net::TcpListener;
use tokio_tungstenite::{accept_async, tungstenite::Message};

#[test]
fn maestro_response_sessions_do_not_replay_started_output() {
    let _guard = super::exclusive();
    run(maestro_response_sessions_do_not_replay_started_output_scenario());
}

/// Execute the controlled public-operation scenario.
async fn maestro_response_sessions_do_not_replay_started_output_scenario() {
    for mode in [
        Transport::Websocket,
        Transport::WebsocketCached,
        Transport::Auto,
    ] {
        for text in [false, true] {
            started_case(mode.clone(), text).await;
        }
    }
}

#[test]
fn maestro_response_sessions_default_auto_reuses_context() {
    let _guard = super::exclusive();
    run(maestro_response_sessions_default_auto_reuses_context_scenario());
}

/// Execute the controlled public-operation scenario.
async fn maestro_response_sessions_default_auto_reuses_context_scenario() {
    for route in 0..4 {
        default_context_case(route).await;
    }
}

/// Execute one finite peer script through the public dispatcher.
async fn scripted(
    mode: Transport,
    script: Vec<String>,
    cancel_after_text: bool,
) -> (crate::SharedAssistantMessage, usize, Vec<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (model, context, mut options) = super::invocation();
    let mut model = (*model).clone();
    model.base_url = format!("http://{}", listener.local_addr().unwrap());
    options.common.transport = Some(mode);
    let signal = crate::Cancellation::new();
    options.common.signal = Some(signal.clone());
    let calls = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&calls);
    options.common.fetch = Some(Arc::new(move |_| {
        count.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Ok(super::public_streams::response()) })
    }));
    let (finish, finished) = tokio::sync::oneshot::channel();
    let peer = tokio::spawn(async move {
        let mut socket = accept_async(listener.accept().await.unwrap().0)
            .await
            .unwrap();
        super::loopback::read_request(&mut socket).await;
        for event in script {
            socket.send(Message::text(event)).await.unwrap();
        }
        if cancel_after_text {
            let _ = finished.await;
        }
        let _ = socket.close(None).await;
        while socket.next().await.is_some() {}
    });
    let stream = stream_openai_codex_responses(model, context, Some(options));
    let mut events = Vec::new();
    if cancel_after_text {
        loop {
            let event = stream.next().await.unwrap();
            let delta = matches!(event, crate::AssistantMessageEvent::TextDelta { .. });
            events.push(
                serde_json::to_value(event).unwrap()["type"]
                    .as_str()
                    .unwrap()
                    .into(),
            );
            if delta {
                break;
            }
        }
        signal.abort();
    }
    let _ = finish.send(());
    let output = stream.result().await;
    peer.await.unwrap();
    while let Some(event) = stream.next().await {
        events.push(
            serde_json::to_value(event).unwrap()["type"]
                .as_str()
                .unwrap()
                .into(),
        );
    }
    (output, calls.load(Ordering::SeqCst), events)
}

#[test]
fn maestro_response_sessions_prestart_transport_fallback() {
    let _guard = super::exclusive();
    run(maestro_response_sessions_prestart_transport_fallback_scenario());
}

/// Execute the controlled public-operation scenario.
async fn maestro_response_sessions_prestart_transport_fallback_scenario() {
    for mode in [
        Transport::Websocket,
        Transport::WebsocketCached,
        Transport::Auto,
    ] {
        for script in [vec![], vec!["{\"type\":3}".into()], vec!["{}".into()]] {
            let (output, calls, events) = scripted(mode.clone(), script, false).await;
            assert_eq!(calls, 1);
            assert_eq!(events.iter().filter(|event| *event == "start").count(), 1);
            assert_eq!(output.read().unwrap().stop_reason, StopReason::Stop);
            assert_eq!(
                serde_json::to_value(&*output.read().unwrap()).unwrap()["content"][0]["text"],
                "Visible Ω😀"
            );
            let output = output.read().unwrap();
            assert_eq!(output.diagnostics.as_ref().unwrap().len(), 1);
            assert_eq!(
                output.diagnostics.as_ref().unwrap()[0]
                    .details
                    .as_ref()
                    .unwrap()["fallbackTransport"],
                "sse"
            );
        }
        let (model, context, mut options) = super::invocation();
        let mut model = (*model).clone();
        model.base_url = "ftp://example.invalid".into();
        options.common.transport = Some(mode);
        options.common.fetch = Some(Arc::new(|_| {
            Box::pin(async { Ok(super::public_streams::response()) })
        }));
        let output = stream_openai_codex_responses(model, context, Some(options))
            .result()
            .await;
        assert_eq!(output.read().unwrap().stop_reason, StopReason::Stop);
        assert_eq!(
            output.read().unwrap().diagnostics.as_ref().unwrap()[0]
                .details
                .as_ref()
                .unwrap()["eventsEmitted"],
            false
        );
    }
}

#[test]
fn maestro_response_sessions_suppress_fallback_for_protocol_api_and_abort() {
    let _guard = super::exclusive();
    run(maestro_response_sessions_suppress_fallback_for_protocol_api_and_abort_scenario());
}

/// Execute the controlled public-operation scenario.
async fn maestro_response_sessions_suppress_fallback_for_protocol_api_and_abort_scenario() {
    for mode in [
        Transport::Websocket,
        Transport::WebsocketCached,
        Transport::Auto,
    ] {
        for after in [false, true] {
            for kind in ["api", "protocol"] {
                excluded_case(mode.clone(), after, kind).await;
            }
        }
        let (output, calls, _) = scripted(
            mode.clone(),
            super::socket_transport::opening("abort-kept"),
            true,
        )
        .await;
        assert_eq!(calls, 0);
        assert_eq!(output.read().unwrap().stop_reason, StopReason::Aborted);
        assert!(output.read().unwrap().diagnostics.is_none());
        let (model, context, mut options) = super::invocation();
        let signal = crate::Cancellation::new();
        signal.abort();
        options.common.signal = Some(signal);
        options.common.transport = Some(mode);
        let calls = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&calls);
        options.common.fetch = Some(Arc::new(move |_| {
            count.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Ok(super::public_streams::response()) })
        }));
        let output = stream_openai_codex_responses((*model).clone(), context, Some(options))
            .result()
            .await;
        assert_eq!(
            output.read().unwrap().error_message.as_deref(),
            Some("Request was aborted")
        );
        assert_eq!(output.read().unwrap().stop_reason, StopReason::Aborted);
        assert!(output.read().unwrap().diagnostics.is_none());
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }
}

/// Diagnostic constructor query.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct DiagnosticQuery {
    /// Configured mode.
    transport: Transport,
    /// First selected event was delivered.
    started: bool,
    /// Full compact request text.
    body: String,
}
/// Expected attributed failure.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct DiagnosticExpected {
    /// Complete diagnostic except nondeterministic timestamp.
    diagnostic: serde_json::Value,
}
/// A strict diagnostic query/result.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DiagnosticCase {
    /// Constructor operands.
    input: DiagnosticQuery,
    /// Output.
    expected: DiagnosticExpected,
}

#[test]
fn maestro_response_sessions_preserve_transport_diagnostics() {
    let rows: Vec<DiagnosticCase> =
        super::fixture_rows(include_str!("fixtures/public-diagnostic.json"), &["input"]).unwrap();
    assert_eq!(rows.len(), 8);
    for row in rows {
        let mut options = super::super::OpenAICodexResponsesOptions::<crate::Threaded>::default();
        options.common.transport = Some(row.input.transport);
        let error = crate::DiagnosticErrorInfo {
            name: Some("WebSocketCloseError".into()),
            message: "transport Ω😀".into(),
            code: Some(crate::DiagnosticCode::Number(1009.0)),
            stack: None,
        };
        let body = serde_json::from_str(&row.input.body).unwrap();
        let actual =
            super::super::stream::transport_diagnostic(&options, &body, row.input.started, &error)
                .unwrap();
        let mut actual = serde_json::to_value(actual).unwrap();
        actual.as_object_mut().unwrap().shift_remove("timestamp");
        assert_eq!(actual.as_object().unwrap().len(), 3);
        assert_eq!(row.expected.diagnostic.as_object().unwrap().len(), 3);
        for field in ["type", "error", "details"] {
            assert_eq!(
                crate::providers::json_text::compact_json(&actual[field]).unwrap(),
                crate::providers::json_text::compact_json(&row.expected.diagnostic[field]).unwrap(),
                "diagnostic field {field}"
            );
        }
    }
}

#[test]
fn maestro_response_sessions_dispatch_cache_matrix() {
    let _guard = super::exclusive();
    run(maestro_response_sessions_dispatch_cache_matrix_scenario());
}

/// Execute the controlled public-operation scenario.
async fn maestro_response_sessions_dispatch_cache_matrix_scenario() {
    for mode in [
        None,
        Some(Transport::Sse),
        Some(Transport::Websocket),
        Some(Transport::WebsocketCached),
        Some(Transport::Auto),
    ] {
        for state in [
            "none",
            "matching",
            "busy",
            "headers",
            "body",
            "busy-headers",
        ] {
            matrix_case(mode.clone(), state).await;
        }
    }
}

/// Compare the second public invocation with the connection the first invocation retained.
async fn matrix_case(mode: Option<Transport>, state: &str) {
    let mut case = Matrix::new(state).await;
    let before = super::super::get_openai_codex_web_socket_debug_stats("dispatch-matrix")
        .unwrap_or_default();
    let reuse = !matches!(mode, Some(Transport::Sse))
        && state != "none"
        && !state.starts_with("busy")
        && !state.contains("headers");
    let request = case.send(mode.clone(), state, reuse).await;
    let after = super::super::get_openai_codex_web_socket_debug_stats("dispatch-matrix")
        .unwrap_or_default();
    assert_matrix(&request, (&before, &after), mode.as_ref(), state, reuse);
    case.finish().await;
}

/// Operands and live peers of one dispatch row.
struct Matrix {
    /// Admission endpoint.
    listener: TcpListener,
    /// Caller descriptor.
    model: crate::Model,
    /// Caller history.
    context: crate::Context,
    /// Caller settings.
    options: super::super::OpenAICodexResponsesOptions,
    /// Peer retained by the initial invocation.
    retained: Option<tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>>,
    /// Peer opened by the observed invocation.
    fresh: Option<tokio_tungstenite::WebSocketStream<tokio::net::TcpStream>>,
    /// First operation still owning its lease.
    held: Option<crate::AssistantMessageEventStream>,
}
impl Matrix {
    /// Open the initial retained session when this row needs one.
    async fn new(state: &str) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let (model, context, mut options) = super::invocation();
        let mut model = (*model).clone();
        model.base_url = format!("http://{}", listener.local_addr().unwrap());
        options.common.session_id = Some("dispatch-matrix".into());
        options.common.transport = Some(Transport::Auto);
        super::super::reset_openai_codex_web_socket_debug_stats(Some("dispatch-matrix"));
        let mut case = Self {
            listener,
            model,
            context,
            options,
            retained: None,
            fresh: None,
            held: None,
        };
        if state != "none" {
            case.seed().await;
        }
        if state.starts_with("busy") {
            case.hold().await;
        }
        case
    }

    /// Complete the request whose output becomes a continuation prefix.
    async fn seed(&mut self) {
        let stream = stream_openai_codex_responses(
            self.model.clone(),
            self.context.clone(),
            Some(self.options.clone()),
        );
        let server = async {
            let mut socket = accept_async(self.listener.accept().await.unwrap().0)
                .await
                .unwrap();
            super::loopback::answer(&mut socket, "seed", "seed reply").await;
            socket
        };
        let (socket, reply) = futures_util::future::join(server, stream.result()).await;
        self.retained = Some(socket);
        self.context
            .messages
            .push(crate::Message::Assistant(reply.read().unwrap().clone()));
        self.context.messages.push(
            serde_json::from_value(
                serde_json::json!({"role":"user","content":"next","timestamp":2}),
            )
            .unwrap(),
        );
    }

    /// Await the first request at the peer before admitting the overlapping caller.
    async fn hold(&mut self) {
        let stream = stream_openai_codex_responses(
            self.model.clone(),
            self.context.clone(),
            Some(self.options.clone()),
        );
        super::loopback::read_request(self.retained.as_mut().unwrap()).await;
        self.held = Some(stream);
    }

    /// Observe the selected socket or HTTP request and settle that invocation.
    async fn send(
        &mut self,
        mode: Option<Transport>,
        state: &str,
        reuse: bool,
    ) -> serde_json::Value {
        self.options.common.transport = mode.clone();
        if state.contains("headers") {
            self.options.common.headers = Some(indexmap::IndexMap::from([(
                "x-edition".into(),
                "changed".into(),
            )]));
        }
        edit_matrix_body(&mut self.options, state);
        let captured = Arc::new(std::sync::Mutex::new(None));
        let capture = Arc::clone(&captured);
        self.options.common.fetch = Some(Arc::new(move |request| {
            *capture.lock().unwrap() =
                Some(serde_json::from_slice::<serde_json::Value>(&request.body).unwrap());
            Box::pin(async { Ok(super::public_streams::response()) })
        }));
        let stream = stream_openai_codex_responses(
            self.model.clone(),
            self.context.clone(),
            Some(self.options.clone()),
        );
        if matches!(mode, Some(Transport::Sse)) {
            assert_eq!(
                stream.result().await.read().unwrap().stop_reason,
                StopReason::Stop
            );
            return captured.lock().unwrap().take().unwrap();
        }
        let server = async {
            let socket = if reuse {
                self.retained.as_mut().unwrap()
            } else {
                self.fresh = Some(
                    accept_async(self.listener.accept().await.unwrap().0)
                        .await
                        .unwrap(),
                );
                self.fresh.as_mut().unwrap()
            };
            super::loopback::answer(socket, "second", "second reply").await
        };
        let (request, result) = futures_util::future::join(server, stream.result()).await;
        assert_eq!(result.read().unwrap().stop_reason, StopReason::Stop);
        assert!(captured.lock().unwrap().is_none());
        request
    }

    /// Complete the owning call and drain both peers after synchronous cache close.
    async fn finish(mut self) {
        if let Some(held) = self.held {
            super::loopback::respond(self.retained.as_mut().unwrap(), Some("held"), "held reply")
                .await;
            assert_eq!(
                held.result().await.read().unwrap().stop_reason,
                StopReason::Stop
            );
        }
        super::super::close_openai_codex_web_socket_sessions(Some("dispatch-matrix"));
        if let Some(mut socket) = self.retained {
            while socket.next().await.is_some() {}
        }
        if let Some(mut socket) = self.fresh {
            while socket.next().await.is_some() {}
        }
    }
}

/// Assert the selected request against this row's connection and continuation eligibility.
fn assert_matrix(
    request: &serde_json::Value,
    counts: (
        &super::super::OpenAICodexWebSocketDebugStats,
        &super::super::OpenAICodexWebSocketDebugStats,
    ),
    mode: Option<&Transport>,
    state: &str,
    reuse: bool,
) {
    let (before, after) = counts;
    let sse = matches!(mode, Some(Transport::Sse));
    let delta = reuse && state != "body" && !matches!(mode, Some(Transport::Websocket));
    assert_eq!(
        request.get("previous_response_id").is_some(),
        delta,
        "state {state}"
    );
    if delta {
        assert_eq!(request["previous_response_id"], "seed");
        assert_eq!(request["input"], expected_user_input("next"));
    } else {
        let expected = if state == "none" {
            expected_user_input("hello Ω")
        } else {
            expected_full_input("seed reply", "next")
        };
        assert_eq!(request["input"], expected);
    }
    assert_eq!(after.requests - before.requests, u64::from(!sse));
    assert_eq!(
        after.connections_reused - before.connections_reused,
        u64::from(reuse)
    );
    assert_eq!(
        after.connections_created - before.connections_created,
        u64::from(!sse && !reuse)
    );
    assert_eq!(
        after.delta_requests - before.delta_requests,
        u64::from(delta)
    );
}

#[test]
fn maestro_response_sessions_hook_edits_survive_fallback() {
    let _guard = super::exclusive();
    run(maestro_response_sessions_hook_edits_survive_fallback_scenario());
}

/// Execute the controlled public-operation scenario.
async fn maestro_response_sessions_hook_edits_survive_fallback_scenario() {
    let cases = [
        ("{}", "{\"type\":\"response.create\"}"),
        (
            "{\"z\":1e-7,\"input\":[\"Ω😀\",\"Ω😀\"],\"a\":-0.0}",
            "{\"type\":\"response.create\",\"z\":1e-7,\"input\":[\"Ω😀\",\"Ω😀\"],\"a\":0}",
        ),
    ];
    for (edited, expected_socket) in cases {
        for fallback in [false, true] {
            hook_case(edited, expected_socket, fallback).await;
        }
    }
    cached_hook_failure().await;
}

/// Exercise one started transport without HTTP replay.
async fn started_case(mode: Transport, text: bool) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (model, context, mut options) = super::invocation();
    let mut model = (*model).clone();
    model.base_url = format!("http://{}", listener.local_addr().unwrap());
    options.common.transport = Some(mode.clone());
    let calls = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&calls);
    options.common.fetch = Some(Arc::new(move |_| {
        count.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Ok(super::public_streams::response()) })
    }));
    let (finished, done) = tokio::sync::oneshot::channel();
    let peer = tokio::spawn(started_peer(listener, done, text));
    let stream = stream_openai_codex_responses(model, context, Some(options));
    let output = stream.result().await;
    let _ = finished.send(());
    peer.await.unwrap();
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "started socket must not replay over HTTP"
    );
    assert_eq!(output.read().unwrap().stop_reason, StopReason::Error);
    assert_started_diagnostic(&output);
    if text {
        assert_eq!(
            serde_json::to_value(&*output.read().unwrap()).unwrap()["content"][0]["text"],
            "Visible Ω😀"
        );
    }
    let mut starts = 0;
    while let Some(event) = stream.next().await {
        starts += usize::from(matches!(event, crate::AssistantMessageEvent::Start { .. }));
    }
    assert_eq!(starts, 1);
}

/// A started transport failure is attributed after the stream start and never falls back.
fn assert_started_diagnostic(output: &crate::SharedAssistantMessage) {
    let output = output.read().unwrap();
    let details = output.diagnostics.as_ref().unwrap()[0]
        .details
        .as_ref()
        .unwrap();
    assert_eq!(output.diagnostics.as_ref().unwrap().len(), 1);
    assert_eq!(details["eventsEmitted"], true);
    assert_eq!(details["phase"], "after_message_stream_start");
    assert!(!details.contains_key("fallbackTransport"));
}

/// Observe three default calls on the same retained context.
async fn default_context_case(route: u8) {
    super::super::reset_openai_codex_web_socket_debug_stats(None);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (model, mut context, mut options) = super::invocation();
    let mut model = (*model).clone();
    model.id = "controlled-session".into();
    model.base_url = format!("http://{}", listener.local_addr().unwrap());
    options.common.session_id = Some("default-context".into());
    let peer = tokio::spawn(async move {
        let mut socket = accept_async(listener.accept().await.unwrap().0)
            .await
            .unwrap();
        let mut sent = Vec::new();
        for id in ["r1", "r2", "r3"] {
            sent.push(super::loopback::answer(&mut socket, id, "reply").await);
        }
        while socket.next().await.is_some() {}
        sent
    });
    let invoke = |context| context_stream(&model, context, &options, route);
    let first = invoke(context.clone()).result().await;
    context
        .messages
        .push(crate::Message::Assistant(first.read().unwrap().clone()));
    context.messages.push(
        serde_json::from_value(
            serde_json::json!({"role":"user","content":"next Ω😀","timestamp":1}),
        )
        .unwrap(),
    );
    let second = invoke(context.clone()).result().await;
    context
        .messages
        .push(crate::Message::Assistant(second.read().unwrap().clone()));
    let third = invoke(context).result().await;
    assert_eq!(third.read().unwrap().stop_reason, StopReason::Stop);
    assert_eq!(
        serde_json::to_value(&*third.read().unwrap()).unwrap()["content"][0]["text"],
        "reply"
    );
    super::super::close_openai_codex_web_socket_sessions(Some("default-context"));
    let sent = peer.await.unwrap();
    assert_default_context(&sent);
}

/// Check an API or protocol failure before or after start.
async fn excluded_case(mode: Transport, after: bool, kind: &str) {
    let api = kind == "api";
    let mut script = if after {
        super::socket_transport::opening("kept")
    } else {
        vec![]
    };
    script.push(if api {
        "{\"type\":\"error\",\"message\":\"denied\"}".into()
    } else {
        "invalid JSON".into()
    });
    let (output, calls, _) = scripted(mode.clone(), script, false).await;
    let output = output.read().unwrap();
    assert_eq!(calls, 0);
    assert!(output.diagnostics.is_none());
    assert_eq!(output.stop_reason, StopReason::Error);
    if api {
        assert_eq!(output.error_message.as_deref(), Some("Codex error: denied"));
    } else {
        assert!(
            output
                .error_message
                .as_deref()
                .unwrap()
                .starts_with("Invalid Codex WebSocket JSON:")
        );
    }
    if after {
        assert_eq!(
            serde_json::to_value(&*output).unwrap()["content"][0]["text"],
            "kept"
        );
    }
}

/// Compare the retained object payload on socket and HTTP.
async fn hook_case(edited: &'static str, expected_socket: &str, fallback: bool) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (model, context, mut options) = super::invocation();
    let mut model = (*model).clone();
    model.base_url = format!("http://{}", listener.local_addr().unwrap());
    options.common.transport = Some(Transport::WebsocketCached);
    let receiver = hook_observations(&mut options, edited);
    let peer = tokio::spawn(async move {
        let mut socket = accept_async(listener.accept().await.unwrap().0)
            .await
            .unwrap();
        let request = socket
            .next()
            .await
            .unwrap()
            .unwrap()
            .into_text()
            .unwrap()
            .to_string();
        if fallback {
            let _ = socket.close(None).await;
        } else {
            super::loopback::respond(&mut socket, None, "socket reply").await;
        }
        while socket.next().await.is_some() {}
        request
    });
    let output = stream_openai_codex_responses(model, context, Some(options))
        .result()
        .await;
    let request = peer.await.unwrap();
    assert_eq!(request, expected_socket);
    assert_eq!(output.read().unwrap().stop_reason, StopReason::Stop);
    let seen: Vec<_> = receiver.try_iter().collect();
    let kinds: Vec<_> = seen.iter().map(|(kind, _)| *kind).collect();
    assert_eq!(
        kinds,
        if fallback {
            vec!["payload", "fetch", "response"]
        } else {
            vec!["payload"]
        }
    );
    if fallback {
        let expected = if edited.ends_with("-0.0}") {
            "{\"z\":1e-7,\"input\":[\"Ω😀\",\"Ω😀\"],\"a\":0}"
        } else {
            edited
        };
        assert_eq!(seen[1].1, expected);
        assert_eq!(
            output.read().unwrap().diagnostics.as_ref().unwrap()[0]
                .details
                .as_ref()
                .unwrap()["requestBytes"],
            expected.len()
        );
    }
}

/// Publish the selected opening and then close the real peer.
async fn started_peer(listener: TcpListener, done: tokio::sync::oneshot::Receiver<()>, text: bool) {
    let accepted = futures_util::future::select(Box::pin(listener.accept()), done).await;
    let futures_util::future::Either::Left((accepted, _)) = accepted else {
        return;
    };
    let (tcp, _) = accepted.unwrap();
    let mut socket = accept_async(tcp).await.unwrap();
    super::loopback::read_request(&mut socket).await;
    if text {
        for event in super::loopback::response_events(None, "Visible Ω😀")
            .into_iter()
            .take(4)
        {
            socket.send(Message::text(event)).await.unwrap();
        }
    } else {
        socket
            .send(Message::text("{\"type\":\"future.event\"}"))
            .await
            .unwrap();
    }
    socket.close(None).await.unwrap();
    while socket.next().await.is_some() {}
}

/// Install stateful payload/response observers and the controlled HTTP transport.
fn hook_observations(
    options: &mut super::super::OpenAICodexResponsesOptions,
    edited: &'static str,
) -> std::sync::mpsc::Receiver<(&'static str, String)> {
    let (sender, receiver) = std::sync::mpsc::channel();
    let payload_seen = sender.clone();
    options.common.on_payload = Some(Arc::new(move |_, _| {
        payload_seen.send(("payload", String::new())).unwrap();
        Box::pin(async move {
            Ok(crate::PayloadHookInput {
                payload: serde_json::from_str(edited).unwrap(),
                signal: None,
            })
        })
    }));
    let response_seen = sender.clone();
    options.common.on_response = Some(Arc::new(move |_, _| {
        response_seen.send(("response", String::new())).unwrap();
        Box::pin(async { Ok(()) })
    }));
    options.common.fetch = Some(Arc::new(move |request| {
        sender
            .send(("fetch", String::from_utf8(request.body).unwrap()))
            .unwrap();
        Box::pin(async { Ok(super::public_streams::response()) })
    }));
    receiver
}

/// Verify the full request, nonempty delta and empty delta on one peer.
fn assert_default_context(sent: &[serde_json::Value]) {
    assert!(sent[0].get("previous_response_id").is_none());
    assert_eq!(sent[1]["previous_response_id"], "r1");
    assert_eq!(sent[0]["input"], expected_user_input("hello Ω"));
    assert_eq!(sent[1]["input"], expected_user_input("next Ω😀"));
    assert_eq!(sent[2]["previous_response_id"], "r2");
    assert_eq!(sent[2]["input"], serde_json::json!([]));
    assert!(sent.iter().all(|body| body["store"] == false));
    let stats = super::super::get_openai_codex_web_socket_debug_stats("default-context").unwrap();
    assert_eq!(
        (
            stats.connections_created,
            stats.connections_reused,
            stats.delta_requests
        ),
        (1, 2, 2)
    );
}

/// Edit only the body-mismatch row before preparation.
fn edit_matrix_body(options: &mut super::super::OpenAICodexResponsesOptions, state: &str) {
    if state == "body" {
        options.common.on_payload = Some(Arc::new(
            |crate::PayloadHookInput {
                 payload: mut body,
                 signal: hook_signal,
             },
             _| {
                body["changed"] = serde_json::json!(true);
                Box::pin(async {
                    Ok(crate::PayloadHookInput {
                        payload: body,
                        signal: hook_signal,
                    })
                })
            },
        ));
    }
}

/// A cached delta failure must diagnose and replay the full prepared document.
async fn cached_hook_failure() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (model, mut context, mut options) = super::invocation();
    let mut model = (*model).clone();
    model.base_url = format!("http://{}", listener.local_addr().unwrap());
    options.common.session_id = Some("hook-delta".into());
    let (sender, receiver) = std::sync::mpsc::channel();
    options.common.fetch = Some(Arc::new(move |request| {
        sender.send(request.body).unwrap();
        Box::pin(async { Ok(super::public_streams::response()) })
    }));
    let hooks = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&hooks);
    options.common.on_payload = Some(Arc::new(move |mut input, _| {
        count.fetch_add(1, Ordering::SeqCst);
        input.payload["ordered"] = serde_json::json!([2, 1, 2]);
        Box::pin(async { Ok(input) })
    }));
    let peer = tokio::spawn(cached_failure_peer(listener));
    let first =
        stream_openai_codex_responses(model.clone(), context.clone(), Some(options.clone()))
            .result()
            .await;
    context
        .messages
        .push(crate::Message::Assistant(first.read().unwrap().clone()));
    context.messages.push(
        serde_json::from_value(
            serde_json::json!({"role":"user","content":"next Ω😀","timestamp":2}),
        )
        .unwrap(),
    );
    let output = stream_openai_codex_responses(model, context, Some(options))
        .result()
        .await;
    let delta = peer.await.unwrap();
    let full = receiver.recv().unwrap();
    let request: serde_json::Value = serde_json::from_slice(&full).unwrap();
    assert_eq!(
        request["input"],
        expected_full_input("first reply", "next Ω😀")
    );
    assert_eq!(request["ordered"], serde_json::json!([2, 1, 2]));
    assert_eq!(hooks.load(Ordering::SeqCst), 2);
    let message = output.read().unwrap();
    let detail = message.diagnostics.as_ref().unwrap()[0]
        .details
        .as_ref()
        .unwrap();
    assert_eq!(detail["requestBytes"], full.len());
    assert_ne!(full.len(), delta.len());
    assert_eq!(message.stop_reason, StopReason::Stop);
    drop(message);
    super::super::close_openai_codex_web_socket_sessions(Some("hook-delta"));
}

/// Read the cached suffix, then fail before any response event is selected.
async fn cached_failure_peer(listener: TcpListener) -> String {
    let mut socket = accept_async(listener.accept().await.unwrap().0)
        .await
        .unwrap();
    super::loopback::answer(&mut socket, "seed", "first reply").await;
    let delta = socket
        .next()
        .await
        .unwrap()
        .unwrap()
        .into_text()
        .unwrap()
        .to_string();
    let body: serde_json::Value = serde_json::from_str(&delta).unwrap();
    assert_eq!(body["previous_response_id"], "seed");
    assert_eq!(body["input"], expected_user_input("next Ω😀"));
    socket.close(None).await.unwrap();
    while socket.next().await.is_some() {}
    delta
}

/// Invoke the same descriptor through a dedicated or generic raw/simple path.
fn context_stream(
    model: &crate::Model,
    context: crate::Context,
    options: &super::super::OpenAICodexResponsesOptions,
    route: u8,
) -> crate::AssistantMessageEventStream {
    let simple = Some(crate::SimpleStreamOptions {
        common: options.common.clone(),
        ..Default::default()
    });
    match route {
        0 => stream_openai_codex_responses(model.clone(), context, Some(options.clone())),
        1 => super::super::stream_simple_openai_codex_responses(model.clone(), context, simple)
            .unwrap(),
        2 => crate::stream(
            model.clone(),
            context,
            Some(crate::ProviderStreamOptions {
                common: options.common.clone(),
                ..Default::default()
            }),
        )
        .unwrap(),
        _ => crate::stream_simple(model.clone(), context, simple).unwrap(),
    }
}

/// Literal user wire item, independent of production conversion.
fn expected_user_input(text: &str) -> serde_json::Value {
    serde_json::json!([{"role":"user","content":[{"type":"input_text","text":text}]}])
}
/// Complete retained history with distinguishable user and assistant payloads.
fn expected_full_input(reply: &str, next: &str) -> serde_json::Value {
    serde_json::json!([
        {"role":"user","content":[{"type":"input_text","text":"hello Ω"}]},
        {"type":"message","role":"assistant","content":[{"type":"output_text","text":reply,"annotations":[]}],"status":"completed","id":"m1"},
        {"role":"user","content":[{"type":"input_text","text":next}]}
    ])
}
