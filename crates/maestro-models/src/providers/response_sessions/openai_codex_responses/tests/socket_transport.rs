//! Response-session socket witnesses against real loopback peers.

use super::super::request::{PreparedRequest, prepare_request};
use super::super::websocket::{WebSocketOutput, process_web_socket_stream, wire_body};
use super::super::{CodexError, OpenAICodexResponsesOptions};
use super::sockets::{Fault, Incoming, run_script};
use crate::{
    AssistantMessageEvent, AssistantMessageEventStream, Cancellation, Model, SharedAssistantMessage,
};
use futures_util::future::{Either, join, select};
use futures_util::{FutureExt, SinkExt, StreamExt};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use tokio_tungstenite::tungstenite::protocol::frame::Frame;
use tokio_tungstenite::tungstenite::protocol::frame::coding::{Data, OpCode};
use tokio_tungstenite::tungstenite::{Message, handshake::derive_accept_key};
use tokio_tungstenite::{WebSocketStream, accept_async};

/// Run on a real-time runtime so loopback readiness is never mistaken for idleness.
fn run_native<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(future)
}

/// Everything one operation needs.
struct Setup {
    /// Descriptor pointing at the loopback peer.
    model: Arc<Model>,
    /// Options with the credential.
    options: OpenAICodexResponsesOptions,
    /// Prepared request.
    prepared: PreparedRequest,
}

/// Prepare a request for a peer at `base_url`, letting `adjust` change the options first.
async fn setup(base_url: &str, adjust: impl FnOnce(&mut OpenAICodexResponsesOptions)) -> Setup {
    let (model, context, mut options) = super::invocation();
    let mut model = (*model).clone();
    model.base_url = base_url.to_owned();
    let model = Arc::new(model);
    adjust(&mut options);
    let prepared = prepare_request(&model, &context, &options, "maestro (fixture)")
        .await
        .unwrap();
    Setup {
        model,
        options,
        prepared,
    }
}

/// What one operation produced.
struct Outcome {
    /// Operation outcome.
    result: Result<(), CodexError>,
    /// The filled message.
    output: SharedAssistantMessage,
    /// Producer events.
    events: AssistantMessageEventStream,
}

/// Run the operation with the given request identifier, counting start notifications.
async fn operate(setup: &Setup, request_id: &str, starts: &AtomicUsize) -> Outcome {
    let output = Arc::new(std::sync::RwLock::new(
        crate::providers::assistant_output::initial_message(&setup.model),
    ));
    let events = AssistantMessageEventStream::new();
    let mut on_start = || {
        starts.fetch_add(1, Ordering::SeqCst);
    };
    let result = process_web_socket_stream(
        &setup.prepared,
        &setup.model,
        &setup.options,
        WebSocketOutput {
            output: &output,
            stream: &events,
            on_start: &mut on_start,
        },
        request_id,
    )
    .await;
    Outcome {
        result,
        output,
        events,
    }
}

/// Bind a loopback listener and name its endpoint.
async fn listen() -> (TcpListener, String) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    (listener, url)
}

/// Accept one socket and complete the server side of the upgrade.
async fn accept(listener: &TcpListener) -> WebSocketStream<TcpStream> {
    let (stream, _) = listener.accept().await.unwrap();
    accept_async(stream).await.unwrap()
}

/// The visible text of the message.
fn text_of(output: &SharedAssistantMessage) -> String {
    let message = serde_json::to_value(output.read().unwrap().clone()).unwrap();
    message["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_owned()
}

/// Events that precede visible text, then the first delta.
fn opening(delta: &str) -> Vec<String> {
    [
        json!({"type":"response.created","response":{"id":"r1"}}),
        json!({"type":"response.output_item.added","item":{"type":"message","id":"m1","role":"assistant","status":"in_progress","content":[]}}),
        json!({"type":"response.content_part.added","part":{"type":"output_text","text":""}}),
        json!({"type":"response.output_text.delta","delta":delta}),
    ]
    .iter()
    .map(Value::to_string)
    .collect()
}

/// The closing events after the visible text.
fn closing(text: &str) -> Vec<String> {
    [
        json!({"type":"response.output_item.done","item":{"type":"message","id":"m1","role":"assistant","status":"completed","content":[{"type":"output_text","text":text}]}}),
        json!({"type":"response.completed","response":{"id":"r1","status":"completed"}}),
    ]
    .iter()
    .map(Value::to_string)
    .collect()
}

/// Send a ping and wait for its pong, which proves the peer consumed everything sent before it.
async fn acknowledged(socket: &mut WebSocketStream<TcpStream>) {
    socket.send(Message::Ping(vec![1].into())).await.unwrap();
    while let Some(message) = socket.next().await {
        if matches!(message, Ok(Message::Pong(_))) {
            return;
        }
    }
    panic!("peer closed before answering the ping");
}

/// Read raw bytes until the peer closes or vanishes.
async fn drain(stream: &TcpStream) {
    let mut buffer = [0_u8; 4096];
    loop {
        stream.readable().await.unwrap();
        match stream.try_read(&mut buffer) {
            Ok(0) => break,
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(_) => break,
        }
    }
}

/// Read until the peer closes or vanishes, returning the close frame it sent, if any.
async fn until_released(socket: &mut WebSocketStream<TcpStream>) -> Option<(u16, String)> {
    while let Some(message) = socket.next().await {
        match message {
            Ok(Message::Close(frame)) => {
                return frame.map(|frame| (frame.code.into(), frame.reason.as_str().to_owned()));
            }
            Ok(_) => {}
            Err(_) => return None,
        }
    }
    None
}

#[test]
fn maestro_response_sessions_send_authenticated_native_handshake() {
    run_native(async {
        let (url, peer) = raw_peer(closing("")[1..].to_vec());
        let ready = setup(&url, |options| {
            options.common.headers = Some(
                [("X-Custom".to_owned(), "caf\u{e9}".to_owned())]
                    .into_iter()
                    .collect(),
            );
        })
        .await;
        let token = ready.options.common.api_key.clone().unwrap();
        let outcome = operate(&ready, "req-native", &AtomicUsize::new(0)).await;
        outcome.result.unwrap();
        let head = peer.head.recv().unwrap();
        let first_line = head.split(|byte| *byte == b'\r').next().unwrap();
        assert_eq!(first_line, b"GET /codex/responses HTTP/1.1");
        let bearer = format!("Bearer {token}");
        let expected: [(&str, &[u8]); 8] = [
            ("authorization", bearer.as_bytes()),
            ("chatgpt-account-id", b"acc_test"),
            ("openai-beta", b"responses_websockets=2026-02-06"),
            ("session_id", b"req-native"),
            ("x-client-request-id", b"req-native"),
            ("originator", b"maestro"),
            ("user-agent", b"maestro (fixture)"),
            ("x-custom", b"caf\xe9"),
        ];
        for (name, value) in expected {
            assert_eq!(header_values(&head, name), [value], "{name}");
        }
        for absent in ["accept", "content-type"] {
            assert!(header_values(&head, absent).is_empty(), "{absent}");
        }
        peer.finish();
    });
}

#[test]
fn maestro_response_sessions_start_only_after_mapped_event() {
    run_native(async {
        let (listener, url) = listen().await;
        let ready = setup(&url, |_| {}).await;
        let starts = AtomicUsize::new(0);
        let server = async {
            let mut socket = accept(&listener).await;
            socket.next().await.unwrap().unwrap();
            let after_send = starts.load(Ordering::SeqCst);
            for untyped in ["{}", r#"{"type":""}"#, r#"{"type":7}"#, "[]"] {
                socket.send(Message::text(untyped)).await.unwrap();
            }
            acknowledged(&mut socket).await;
            let after_untyped = starts.load(Ordering::SeqCst);
            socket
                .send(Message::text(r#"{"type":"future.event"}"#))
                .await
                .unwrap();
            acknowledged(&mut socket).await;
            let after_unknown = starts.load(Ordering::SeqCst);
            socket
                .send(Message::text(closing("")[1].clone()))
                .await
                .unwrap();
            (after_send, after_untyped, after_unknown)
        };
        let (observed, outcome) = join(server, operate(&ready, "req", &starts)).await;
        outcome.result.unwrap();
        assert_eq!(observed, (0, 0, 1));
        assert_eq!(starts.load(Ordering::SeqCst), 1);

        let terminal_only = async {
            let mut socket = accept(&listener).await;
            socket.next().await.unwrap().unwrap();
            socket
                .send(Message::text(closing("")[1].clone()))
                .await
                .unwrap();
        };
        let starts = AtomicUsize::new(0);
        let ((), outcome) = join(terminal_only, operate(&ready, "req", &starts)).await;
        outcome.result.unwrap();
        assert_eq!(starts.load(Ordering::SeqCst), 1);
    });
}

#[test]
fn maestro_response_sessions_finish_socket_before_peer_close() {
    run_native(async {
        let (listener, url) = listen().await;
        let ready = setup(&url, |_| {}).await;
        for (terminal, status, reason) in [
            ("response.completed", "completed", "stop"),
            ("response.done", "completed", "stop"),
            ("response.incomplete", "incomplete", "length"),
        ] {
            let (done_tx, done_rx) = oneshot::channel::<()>();
            let server = async {
                let mut socket = accept(&listener).await;
                socket.next().await.unwrap().unwrap();
                let event = json!({"type":terminal,"response":{"id":"r1","status":status}});
                socket.send(Message::text(event.to_string())).await.unwrap();
                socket.send(Message::text("{bad}")).await.unwrap();
                done_rx.await.unwrap();
            };
            let client = async {
                let outcome = operate(&ready, "req", &AtomicUsize::new(0)).await;
                done_tx.send(()).unwrap();
                outcome
            };
            let ((), outcome) = join(server, client).await;
            outcome.result.unwrap();
            let message = serde_json::to_value(outcome.output.read().unwrap().clone()).unwrap();
            assert_eq!(message["stopReason"], reason);
        }
    });
}

/// Send one payload as two fragments of the given data kind.
async fn send_fragmented(socket: &mut WebSocketStream<TcpStream>, text: &str, kind: Data) {
    let (head, tail) = text.as_bytes().split_at(text.len() / 2);
    let first = Frame::message(head.to_vec(), OpCode::Data(kind), false);
    let last = Frame::message(tail.to_vec(), OpCode::Data(Data::Continue), true);
    socket.send(Message::Frame(first)).await.unwrap();
    socket.send(Message::Frame(last)).await.unwrap();
}

#[test]
fn maestro_response_sessions_retain_socket_receive_order() {
    run_native(async {
        let (listener, url) = listen().await;
        let ready = setup(&url, |_| {}).await;
        let server = async {
            let mut socket = accept(&listener).await;
            socket.next().await.unwrap().unwrap();
            let head = opening("A");
            for event in &head[..3] {
                socket.send(Message::text(event.clone())).await.unwrap();
            }
            send_fragmented(&mut socket, &head[3], Data::Text).await;
            acknowledged(&mut socket).await;
            let second = json!({"type":"response.output_text.delta","delta":"B"}).to_string();
            socket
                .send(Message::binary(second.into_bytes()))
                .await
                .unwrap();
            acknowledged(&mut socket).await;
            let third = json!({"type":"response.output_text.delta","delta":"C"}).to_string();
            send_fragmented(&mut socket, &third, Data::Binary).await;
            acknowledged(&mut socket).await;
            for event in closing("ABC") {
                socket.send(Message::text(event)).await.unwrap();
            }
        };
        let ((), outcome) = join(server, operate(&ready, "req", &AtomicUsize::new(0))).await;
        outcome.result.unwrap();
        assert_eq!(text_of(&outcome.output), "ABC");
        let mut deltas = Vec::new();
        let mut start = None;
        while let Some(Some(event)) = outcome.events.next().now_or_never() {
            match event {
                AssistantMessageEvent::Start { partial } => start = Some(partial),
                AssistantMessageEvent::TextDelta { delta, .. } => deltas.push(delta),
                _ => {}
            }
        }
        assert_eq!(deltas, ["A", "B", "C"]);
        let start = start.expect("start event");
        assert!(
            Arc::ptr_eq(&start, &outcome.output),
            "start handle is a copy"
        );
        assert_eq!(text_of(&start), "ABC");
    });
}

/// A server frame carrying `text`, unmasked as servers send it.
fn server_frame(text: &str) -> Vec<u8> {
    let mut frame = vec![0x81];
    match u8::try_from(text.len()) {
        Ok(length) if length < 126 => frame.push(length),
        _ => {
            frame.push(126);
            frame.extend(u16::try_from(text.len()).unwrap().to_be_bytes());
        }
    }
    frame.extend(text.as_bytes());
    frame
}

/// Read a request head, which ends at the first blank line.
fn read_head(stream: &mut std::net::TcpStream) -> Vec<u8> {
    let mut head = Vec::new();
    let mut byte = [0_u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        stream.read_exact(&mut byte).unwrap();
        head.push(byte[0]);
    }
    head
}

/// Every value of the named header in a request head, as the raw bytes that were sent.
fn header_values<'a>(head: &'a [u8], name: &str) -> Vec<&'a [u8]> {
    head.split(|byte| *byte == b'\n')
        .filter_map(|line| {
            let line = line.strip_suffix(b"\r").unwrap_or(line);
            let colon = line.iter().position(|byte| *byte == b':')?;
            let (key, value) = line.split_at(colon);
            key.eq_ignore_ascii_case(name.as_bytes())
                .then(|| value[1..].trim_ascii())
        })
        .collect()
}

/// A raw peer: its request head, and an acknowledgement after the client released the connection.
struct RawPeer {
    /// The upgrade request head as received.
    head: mpsc::Receiver<Vec<u8>>,
    /// Sent once the client closed the connection.
    released: mpsc::Receiver<()>,
    /// The thread serving the connection.
    worker: std::thread::JoinHandle<()>,
}

impl RawPeer {
    /// Wait for the client's release acknowledgement, then join the serving thread.
    fn finish(self) {
        self.released.recv().unwrap();
        self.worker.join().unwrap();
    }
}

/// Accept the upgrade and write the response with every event in one write.
fn raw_peer(events: Vec<String>) -> (String, RawPeer) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let (head_tx, head) = mpsc::channel();
    let (released_tx, released) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let head = read_head(&mut stream);
        let key = header_values(&head, "sec-websocket-key")[0].to_vec();
        let mut reply = format!(
            "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n\r\n",
            derive_accept_key(&key)
        )
        .into_bytes();
        for event in &events {
            reply.extend(server_frame(event));
        }
        head_tx.send(head).unwrap();
        stream.write_all(&reply).unwrap();
        let mut rest = Vec::new();
        stream.read_to_end(&mut rest).ok();
        released_tx.send(()).unwrap();
    });
    (
        url,
        RawPeer {
            head,
            released,
            worker,
        },
    )
}

#[test]
fn maestro_response_sessions_receive_immediate_socket_response() {
    run_native(async {
        let events = opening("now").into_iter().chain(closing("now")).collect();
        let (url, peer) = raw_peer(events);
        let ready = setup(&url, |_| {}).await;
        let outcome = operate(&ready, "req", &AtomicUsize::new(0)).await;
        outcome.result.unwrap();
        assert_eq!(text_of(&outcome.output), "now");
        peer.finish();
    });
}

#[test]
fn maestro_response_sessions_cancel_socket_connect() {
    run_native(async {
        let (listener, url) = listen().await;
        let mut ready = setup(&url, |_| {}).await;
        let signal = Cancellation::new();
        signal.abort();
        ready.options.common.signal = Some(signal);
        let starts = AtomicUsize::new(0);
        let aborted = operate(&ready, "req", &starts).await;
        let CodexError::Transport(info) = aborted.result.unwrap_err() else {
            panic!("transport failure expected");
        };
        assert_eq!(info.message, "Request was aborted");
        assert!(
            listener.accept().now_or_never().is_none(),
            "connected despite abort"
        );

        let invalid = operate(&ready, "bad\nid", &starts)
            .await
            .result
            .unwrap_err();
        let CodexError::Transport(info) = invalid else {
            panic!("transport failure expected");
        };
        assert_ne!(info.message, "Request was aborted");

        let pending = Cancellation::new();
        ready.options.common.signal = Some(pending.clone());
        let (accepted_tx, accepted_rx) = oneshot::channel::<()>();
        let (released_tx, released_rx) = oneshot::channel::<()>();
        let server = async {
            let (stream, _) = listener.accept().await.unwrap();
            accepted_tx.send(()).unwrap();
            drain(&stream).await;
            released_tx.send(()).unwrap();
        };
        let driver = async {
            accepted_rx.await.unwrap();
            pending.abort();
        };
        let client = operate(&ready, "req", &starts);
        let ((), (), outcome) = futures_util::future::join3(server, driver, client).await;
        released_rx.await.unwrap();
        assert!(
            matches!(outcome.result, Err(CodexError::Transport(info)) if info.message == "Request was aborted")
        );
        assert_eq!(starts.load(Ordering::SeqCst), 0);
    });
}

#[test]
fn maestro_response_sessions_cancel_socket_read() {
    run_native(async {
        let (listener, url) = listen().await;
        let mut ready = setup(&url, |_| {}).await;
        let signal = Cancellation::new();
        ready.options.common.signal = Some(signal.clone());
        let (consumed_tx, consumed_rx) = oneshot::channel::<()>();
        let server = async {
            let mut socket = accept(&listener).await;
            socket.next().await.unwrap().unwrap();
            for event in opening("kept") {
                socket.send(Message::text(event)).await.unwrap();
            }
            acknowledged(&mut socket).await;
            consumed_tx.send(()).unwrap();
            until_released(&mut socket).await
        };
        let driver = async {
            consumed_rx.await.unwrap();
            signal.abort();
        };
        let starts = AtomicUsize::new(0);
        let client = operate(&ready, "req", &starts);
        let (released, (), outcome) = futures_util::future::join3(server, driver, client).await;
        assert!(
            matches!(outcome.result, Err(CodexError::Transport(info)) if info.message == "Request was aborted")
        );
        assert_eq!(text_of(&outcome.output), "kept");
        assert_eq!(released, Some((1000, "done".to_owned())));
    });
}

/// Serve two connections, answering the first with a terminal event and the second with an error.
async fn serve_success_then_error(listener: &TcpListener) -> Vec<Option<(u16, String)>> {
    let mut closes = Vec::new();
    let replies = [
        closing("")[1].clone(),
        json!({"type":"error","message":"no"}).to_string(),
    ];
    for reply in replies {
        let mut socket = accept(listener).await;
        socket.next().await.unwrap().unwrap();
        socket.send(Message::text(reply)).await.unwrap();
        closes.push(until_released(&mut socket).await);
    }
    closes
}

#[test]
fn maestro_response_sessions_close_uncached_socket() {
    run_native(async {
        for session in [None, Some(""), Some("session")] {
            let (listener, url) = listen().await;
            let ready = setup(&url, |options| {
                options.common.session_id = session.map(str::to_owned);
            })
            .await;
            let client = async {
                let first = operate(&ready, "req", &AtomicUsize::new(0)).await.result;
                let second = operate(&ready, "req", &AtomicUsize::new(0)).await.result;
                (first, second)
            };
            let (closes, (first, second)) = join(serve_success_then_error(&listener), client).await;
            assert!(first.is_ok(), "{session:?}");
            assert!(matches!(second, Err(CodexError::Api(_))), "{session:?}");
            let released = Some((1000, "done".to_owned()));
            assert_eq!(closes, [released.clone(), released], "{session:?}");
        }
        let failing = vec![Incoming::Message(Message::text(
            json!({"type":"error","message":"no"}).to_string(),
        ))];
        let run = run_script(failing, Fault::Close).await;
        assert!(
            matches!(run.result, Err(CodexError::Api(_))),
            "close failure replaced the primary result"
        );
        let complete = vec![Incoming::Message(Message::text(closing("")[1].clone()))];
        assert!(run_script(complete, Fault::Close).await.result.is_ok());
    });
}

#[test]
fn maestro_response_sessions_release_socket_on_future_drop() {
    run_native(async {
        let (listener, url) = listen().await;
        let ready = setup(&url, |_| {}).await;
        let (read_tx, read_rx) = oneshot::channel::<()>();
        let (eof_tx, eof_rx) = oneshot::channel::<()>();
        let server = async {
            let mut socket = accept(&listener).await;
            socket.next().await.unwrap().unwrap();
            for event in opening("pending") {
                socket.send(Message::text(event)).await.unwrap();
            }
            acknowledged(&mut socket).await;
            read_tx.send(()).unwrap();
            until_released(&mut socket).await;
            eof_tx.send(()).unwrap();
        };
        let starts = AtomicUsize::new(0);
        let client = async {
            let operation = Box::pin(operate(&ready, "req", &starts));
            match select(operation, read_rx).await {
                Either::Right((read, operation)) => {
                    read.unwrap();
                    drop(operation);
                }
                Either::Left(_) => panic!("operation finished before it was dropped"),
            }
            eof_rx.await.unwrap();
        };
        join(server, client).await;
    });
}

/// Serve a TLS-less peer: report the first bytes the client sends, then hang up.
fn first_bytes_peer() -> (String, mpsc::Receiver<Vec<u8>>, std::thread::JoinHandle<()>) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut head = [0_u8; 4];
        let read = stream.read(&mut head).unwrap();
        tx.send(head[..read].to_vec()).unwrap();
    });
    (addr.to_string(), rx, worker)
}

#[test]
fn maestro_response_sessions_use_secure_socket_transport() {
    run_native(async {
        let (addr, first, worker) = first_bytes_peer();
        let ready = setup(&format!("https://{addr}"), |_| {}).await;
        let outcome = operate(&ready, "req", &AtomicUsize::new(0)).await;
        assert!(matches!(outcome.result, Err(CodexError::Transport(_))));
        let head = first.recv().unwrap();
        worker.join().unwrap();
        assert_eq!(
            head[0], 0x16,
            "a TLS handshake record, not plaintext: {head:?}"
        );
    });
}

/// Counters for the hooks a socket operation must not run again.
#[derive(Default)]
struct HookCounts {
    /// Payload hook calls.
    payloads: AtomicUsize,
    /// Response hook calls.
    responses: AtomicUsize,
    /// Fetch calls.
    fetches: AtomicUsize,
}

/// Count every hook; the payload hook also edits the retained request.
fn count_hooks(options: &mut OpenAICodexResponsesOptions, counts: &Arc<HookCounts>) {
    let seen = Arc::clone(counts);
    options.common.on_payload = Some(Arc::new(move |mut payload, _| {
        seen.payloads.fetch_add(1, Ordering::SeqCst);
        payload["store"] = json!(true);
        payload["edited"] = json!("kept");
        Box::pin(async move { Ok(payload) })
    }));
    let seen = Arc::clone(counts);
    options.common.on_response = Some(Arc::new(move |_, _| {
        seen.responses.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Ok(()) })
    }));
    let seen = Arc::clone(counts);
    options.common.fetch = Some(Arc::new(move |_| {
        seen.fetches.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Err(crate::FetchError::Aborted) })
    }));
}

/// An invalid initial header fails preparation even when a later layer replaces it.
async fn assert_invalid_initial_header_stops(url: &str, listener: &TcpListener) {
    let (model, context, mut options) = super::invocation();
    let mut descriptor = (*model).clone();
    descriptor.base_url = url.to_owned();
    descriptor.headers = Some(
        [("X-Test".to_owned(), "a\nb".to_owned())]
            .into_iter()
            .collect(),
    );
    options.common.headers = Some(
        [("X-Test".to_owned(), "valid".to_owned())]
            .into_iter()
            .collect(),
    );
    let prepared = prepare_request(
        &Arc::new(descriptor),
        &context,
        &options,
        "maestro (fixture)",
    )
    .await;
    assert!(
        prepared.is_err(),
        "invalid initial header survived a valid replacement"
    );
    assert!(
        listener.accept().now_or_never().is_none(),
        "connected during failed preparation"
    );
}

#[test]
fn maestro_response_sessions_keep_socket_hooks_single_use() {
    run_native(async {
        let (listener, url) = listen().await;
        let counts = Arc::new(HookCounts::default());
        let ready = setup(&url, |options| count_hooks(options, &counts)).await;
        let server = async {
            let mut socket = accept(&listener).await;
            let request = socket.next().await.unwrap().unwrap();
            socket
                .send(Message::text(closing("")[1].clone()))
                .await
                .unwrap();
            request.into_text().unwrap().as_str().to_owned()
        };
        let (sent, outcome) = join(server, operate(&ready, "req", &AtomicUsize::new(0))).await;
        outcome.result.unwrap();
        assert_eq!(sent, wire_body(&ready.prepared.body).unwrap());
        assert!(sent.contains(r#""store":true"#) && sent.contains(r#""edited":"kept""#));
        assert_eq!(counts.payloads.load(Ordering::SeqCst), 1);
        assert_eq!(counts.responses.load(Ordering::SeqCst), 0);
        assert_eq!(counts.fetches.load(Ordering::SeqCst), 0);
        assert_invalid_initial_header_stops(&url, &listener).await;
    });
}

/// Answer the next upgrade request with a plain 403 response.
async fn refuse(listener: &TcpListener) {
    const REFUSAL: &[u8] = b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n";
    let (stream, _) = listener.accept().await.unwrap();
    let (mut head, mut buffer) = (Vec::new(), [0_u8; 1]);
    while !head.ends_with(b"\r\n\r\n") {
        stream.readable().await.unwrap();
        match stream.try_read(&mut buffer) {
            Ok(read) => head.extend(&buffer[..read]),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(error) => panic!("{error}"),
        }
    }
    let mut written = 0;
    while written < REFUSAL.len() {
        stream.writable().await.unwrap();
        match stream.try_write(&REFUSAL[written..]) {
            Ok(count) => written += count,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(error) => panic!("{error}"),
        }
    }
}

/// Refuse every upgrade until the operation is done, then report how many were attempted.
async fn refused_attempts(listener: &TcpListener, done: oneshot::Receiver<()>) -> usize {
    let (mut attempts, mut done) = (0, done);
    loop {
        match select(Box::pin(refuse(listener)), done).await {
            Either::Left(((), pending)) => {
                attempts += 1;
                done = pending;
            }
            Either::Right(_) => return attempts,
        }
    }
}

#[test]
fn maestro_response_sessions_reject_failed_socket_handshake() {
    run_native(async {
        let (listener, url) = listen().await;
        let ready = setup(&url, |_| {}).await;
        let starts = AtomicUsize::new(0);
        let (done_tx, done_rx) = oneshot::channel::<()>();
        let server = async { refused_attempts(&listener, done_rx).await };
        let client = async {
            let outcome = operate(&ready, "req", &starts).await;
            done_tx.send(()).unwrap();
            outcome
        };
        let (attempts, outcome) = join(server, client).await;
        assert!(matches!(outcome.result, Err(CodexError::Transport(_))));
        assert_eq!(starts.load(Ordering::SeqCst), 0);
        assert!(
            outcome.events.next().now_or_never().is_none(),
            "model event after a refused upgrade"
        );
        assert_eq!(attempts, 1);
    });
}

#[test]
fn maestro_response_sessions_attempt_socket_upgrade_once() {
    run_native(async {
        let (listener, url) = listen().await;
        let ready = setup(&url, |options| {
            options.common.max_retries = Some(5.0);
            options.common.max_retry_delay_ms = Some(1.0);
        })
        .await;
        let (done_tx, done_rx) = oneshot::channel::<()>();
        let server = async { refused_attempts(&listener, done_rx).await };
        let client = async {
            let outcome = operate(&ready, "req", &AtomicUsize::new(0)).await;
            done_tx.send(()).unwrap();
            outcome
        };
        let (attempts, outcome) = join(server, client).await;
        assert!(matches!(outcome.result, Err(CodexError::Transport(_))));
        assert_eq!(attempts, 1);
    });
}

#[test]
fn maestro_response_sessions_ignore_common_socket_timeout() {
    super::sockets::block_on(async {
        let (listener, url) = listen().await;
        let ready = setup(&url, |options| options.common.timeout_ms = Some(1.0)).await;
        let (gate_tx, gate_rx) = oneshot::channel::<()>();
        let (pending_tx, pending_rx) = oneshot::channel::<()>();
        let server = async {
            let (stream, _) = listener.accept().await.unwrap();
            pending_tx.send(()).unwrap();
            gate_rx.await.unwrap();
            let mut socket = accept_async(stream).await.unwrap();
            socket.next().await.unwrap().unwrap();
            socket
                .send(Message::text(closing("")[1].clone()))
                .await
                .unwrap();
        };
        let starts = AtomicUsize::new(0);
        let mut client = Box::pin(operate(&ready, "req", &starts));
        let driver = async {
            match select(client.as_mut(), pending_rx).await {
                Either::Right((pending, _)) => pending.unwrap(),
                Either::Left(_) => panic!("operation finished before the upgrade was pending"),
            }
            let timeout = std::time::Duration::from_secs(60);
            tokio::time::advance(timeout).await;
            assert!(
                client.as_mut().now_or_never().is_none(),
                "common timeout ended the pending upgrade"
            );
            gate_tx.send(()).unwrap();
            tokio::time::advance(timeout).await;
            client.await
        };
        let ((), outcome) = join(server, driver).await;
        outcome.result.unwrap();
    });
}

/// Run one operation against `base_url` and return its transport failure.
async fn admission_failure(base_url: &str) -> crate::DiagnosticErrorInfo {
    let ready = setup(base_url, |_| {}).await;
    let CodexError::Transport(info) = operate(&ready, "req", &AtomicUsize::new(0))
        .await
        .result
        .unwrap_err()
    else {
        panic!("transport failure expected");
    };
    info
}

#[test]
fn maestro_response_sessions_reject_unconnectable_socket_urls() {
    run_native(async {
        let (listener, url) = listen().await;
        let port = url.rsplit(':').next().unwrap();
        for (base, message) in [
            (
                format!("ftp://127.0.0.1:{port}"),
                "expected a ws: or wss: url",
            ),
            (format!("{url}#fragment"), "hash"),
        ] {
            let info = admission_failure(&base).await;
            assert_eq!(info.name.as_deref(), Some("SyntaxError"), "{base}");
            assert_eq!(info.message, message, "{base}");
            assert!(
                listener.accept().now_or_never().is_none(),
                "connected for {base}"
            );
        }
    });
}

#[test]
fn maestro_response_sessions_keep_encoded_hash_in_socket_path() {
    run_native(async {
        let (url, peer) = raw_peer(closing("")[1..].to_vec());
        let ready = setup(&format!("{url}/a%23b"), |_| {}).await;
        operate(&ready, "req", &AtomicUsize::new(0))
            .await
            .result
            .unwrap();
        let head = peer.head.recv().unwrap();
        let first_line = head.split(|byte| *byte == b'\r').next().unwrap();
        assert_eq!(first_line, b"GET /a%23b/codex/responses HTTP/1.1");
        peer.finish();
    });
}

/// Which native ending a peer produces after reading the request.
#[derive(Clone, Copy)]
enum Ending {
    /// A close frame without a status.
    NoStatus,
    /// The connection drops without a closing handshake.
    Vanish,
}

#[test]
fn maestro_response_sessions_report_native_close_endings() {
    run_native(async {
        for (ending, code) in [(Ending::NoStatus, 1005), (Ending::Vanish, 1006)] {
            let (listener, url) = listen().await;
            let ready = setup(&url, |_| {}).await;
            let server = async {
                let mut socket = accept(&listener).await;
                socket.next().await.unwrap().unwrap();
                match ending {
                    Ending::NoStatus => socket.send(Message::Close(None)).await.unwrap(),
                    Ending::Vanish => drop(socket.into_inner()),
                }
            };
            let ((), outcome) = join(server, operate(&ready, "req", &AtomicUsize::new(0))).await;
            let CodexError::Transport(info) = outcome.result.unwrap_err() else {
                panic!("transport failure expected");
            };
            assert_eq!(info.message, format!("WebSocket closed {code}"));
            assert_eq!(
                info.code,
                Some(crate::DiagnosticCode::Number(f64::from(code)))
            );
        }
    });
}
