//! Socket counters, fallback state and the reset and close controls.

use super::super::continuation::Request;
use super::super::debug::{
    OpenAICodexWebSocketDebugStats as Stats, count_request,
    get_openai_codex_web_socket_debug_stats as stats_of,
    is_web_socket_sse_fallback_active as active, record_web_socket_failure,
    record_web_socket_sse_fallback, reset_openai_codex_web_socket_debug_stats as reset,
};
use super::super::sessions::{cached_entry, close_openai_codex_web_socket_sessions as close};
use super::loopback::{Conversation, answer, read_request, respond};
use super::socket_transport::{accept, acknowledged, listen, run_native, until_released};
use crate::{DiagnosticErrorInfo, DiagnosticInput, Transport};
use futures_util::future::{join, join3};
use serde_json::Value;
use std::sync::{Arc, Mutex};
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio_tungstenite::WebSocketStream;

/// A request body edit.
type Edit = fn(&mut Value);

/// The peer's end of a socket.
type Peer = WebSocketStream<tokio::net::TcpStream>;

/// The counters of a session that must have some.
fn stats(session: &str) -> Stats {
    stats_of(session).unwrap()
}

/// One request of the counting conversation and how it changes the counters.
struct Step {
    /// What the step exercises.
    label: &'static str,
    /// Transport selected for the request.
    transport: Option<Transport>,
    /// Edit applied to the prepared body.
    edit: Option<Edit>,
    /// A user message follows the reply.
    user: bool,
    /// Changes from the previous counters to this request's.
    count: fn(&mut Stats),
}

/// The requests of the counting conversation, each over the one reused connection.
fn steps() -> Vec<Step> {
    let mut all = ordinary_steps();
    all.extend(cached_steps());
    all.extend(edited_steps());
    all
}

/// The first connection and a request without a retained context.
fn ordinary_steps() -> Vec<Step> {
    vec![
        Step {
            label: "ordinary first connection",
            transport: Some(Transport::Websocket),
            edit: None,
            user: true,
            count: |w| {
                w.requests = 1;
                w.connections_created = 1;
                w.full_context_requests = 1;
                w.last_input_items = 1;
            },
        },
        Step {
            label: "cached mode without context",
            transport: Some(Transport::WebsocketCached),
            edit: None,
            user: true,
            count: |w| {
                w.requests = 2;
                w.connections_reused = 1;
                w.cached_context_requests = 1;
                w.full_context_requests = 2;
                w.last_input_items = 3;
            },
        },
    ]
}

/// Cached-context requests that send a suffix.
fn cached_steps() -> Vec<Step> {
    vec![
        Step {
            label: "delta",
            transport: Some(Transport::Auto),
            edit: None,
            user: false,
            count: |w| {
                w.requests = 3;
                w.connections_reused = 2;
                w.cached_context_requests = 2;
                w.delta_requests = 1;
                w.last_input_items = 1;
                w.last_delta_input_items = Some(1);
                w.last_previous_response_id = Some("r2".to_owned());
            },
        },
        Step {
            label: "empty delta",
            transport: Some(Transport::Auto),
            edit: None,
            user: true,
            count: |w| {
                w.requests = 4;
                w.connections_reused = 3;
                w.cached_context_requests = 3;
                w.delta_requests = 2;
                w.last_input_items = 0;
                w.last_delta_input_items = Some(0);
                w.last_previous_response_id = Some("r3".to_owned());
            },
        },
    ]
}

/// Requests whose body the caller edited.
fn edited_steps() -> Vec<Step> {
    vec![
        Step {
            label: "caller identifier in ordinary mode",
            transport: None,
            edit: Some(|payload| payload["previous_response_id"] = "manual".into()),
            user: true,
            count: |w| {
                w.requests = 5;
                w.connections_reused = 4;
                w.delta_requests = 3;
                w.last_input_items = 8;
                w.last_delta_input_items = Some(8);
                w.last_previous_response_id = Some("manual".to_owned());
            },
        },
        Step {
            label: "store true; a full send clears the delta fields",
            transport: Some(Transport::Websocket),
            edit: Some(|payload| payload["store"] = true.into()),
            user: true,
            count: |w| {
                w.requests = 6;
                w.connections_reused = 5;
                w.store_true_requests = 1;
                w.full_context_requests = 3;
                w.last_input_items = 10;
                w.last_delta_input_items = None;
                w.last_previous_response_id = None;
            },
        },
        Step {
            label: "store null",
            transport: Some(Transport::Websocket),
            edit: Some(|payload| payload["store"] = Value::Null),
            user: true,
            count: |w| {
                w.requests = 7;
                w.connections_reused = 6;
                w.full_context_requests = 4;
                w.last_input_items = 12;
            },
        },
    ]
}

/// Answer one request per step on the first connection.
async fn serve_turns(listener: &TcpListener, turns: usize) -> Vec<Value> {
    let mut socket = accept(listener).await;
    let mut sent = Vec::new();
    for turn in 1..=turns {
        sent.push(answer(&mut socket, &format!("r{turn}"), "text").await);
    }
    sent
}

/// Send each step's request and compare every counter after it.
async fn run_steps(chat: &mut Conversation, knob: &Mutex<Option<Edit>>, session: &str) {
    let mut want = Stats::default();
    for step in steps() {
        chat.setup
            .options
            .common
            .transport
            .clone_from(&step.transport);
        *knob.lock().unwrap() = step.edit;
        let outcome = chat.send().await;
        outcome.result.unwrap();
        chat.reply(&outcome.output);
        if step.user {
            chat.user("more");
        }
        (step.count)(&mut want);
        assert_eq!(stats(session), want, "{}", step.label);
    }
}

/// A conversation whose payload hook applies the knob's current edit.
async fn editable(url: &str, session: &str) -> (Conversation, Arc<Mutex<Option<Edit>>>) {
    let mut chat = Conversation::new(url, Some(session), None).await;
    let knob: Arc<Mutex<Option<Edit>>> = Arc::default();
    let read = Arc::clone(&knob);
    chat.setup.options.common.on_payload = Some(Arc::new(move |mut payload, _| {
        if let Some(edit) = *read.lock().unwrap() {
            edit(&mut payload);
        }
        Box::pin(async move { Ok(payload) })
    }));
    (chat, knob)
}

/// A request that never connects counts nothing; one that connects and then fails still counts.
async fn failures_count_by_timing(listener: &TcpListener, url: &str) {
    let refused = {
        let closed = TcpListener::bind("127.0.0.1:0").await.unwrap();
        format!("http://{}", closed.local_addr().unwrap())
    };
    let unreachable = Conversation::new(&refused, Some("count-refused"), None).await;
    assert!(unreachable.send().await.result.is_err());
    assert!(
        stats_of("count-refused").is_none(),
        "a failed acquisition counts nothing"
    );

    let failing = Conversation::new(url, Some("count-failed"), None).await;
    let hang_up = async { drop(accept(listener).await) };
    let (outcome, ()) = join(failing.send(), hang_up).await;
    assert!(outcome.result.is_err());
    let failed = stats("count-failed");
    assert_eq!(
        (failed.requests, failed.connections_created),
        (1, 1),
        "a failed send still counts"
    );
    close(Some("count-failed"));
}

#[test]
fn maestro_response_sessions_count_socket_request_modes() {
    let _isolated = super::exclusive();
    run_native(async {
        let (listener, url) = listen().await;
        let session = "count";
        let (mut chat, knob) = editable(&url, session).await;
        let server = serve_turns(&listener, steps().len());
        let (sent, ()) = join(server, run_steps(&mut chat, &knob, session)).await;
        assert_eq!(sent[4]["previous_response_id"], "manual");
        close(Some(session));
        reset(Some(session));
        failures_count_by_timing(&listener, &url).await;
        reset(None);
    });
}

/// A thrown error with these fields.
fn thrown(name: Option<&str>, message: &str) -> DiagnosticErrorInfo {
    DiagnosticErrorInfo {
        name: name.map(str::to_owned),
        message: message.to_owned(),
        stack: None,
        code: None,
    }
}

/// Counters created by a request, a fallback and a failure keep their optional fields apart.
fn fallback_then_failure() -> (Stats, Stats) {
    let request = Request {
        wire: String::new(),
        input_items: 2,
        previous_response_id: None,
        store_true: false,
    };
    count_request(
        &request,
        &crate::StreamOptions {
            session_id: Some("snapshot".to_owned()),
            ..crate::StreamOptions::default()
        },
        false,
    );
    let counted = stats("snapshot");
    assert_eq!(counted.websocket_fallback_active, None);

    record_web_socket_sse_fallback(Some("snapshot"));
    let fell_back = stats("snapshot");
    assert_eq!(
        (fell_back.sse_fallbacks, fell_back.websocket_fallback_active),
        (1, Some(false))
    );
    assert_eq!(
        counted.sse_fallbacks, 0,
        "an earlier snapshot is independent"
    );

    let error = thrown(Some("TypeError"), "");
    record_web_socket_failure(Some("snapshot"), DiagnosticInput::Error(&error));
    let failed = stats("snapshot");
    assert_eq!(failed.last_web_socket_error.as_deref(), Some("TypeError"));
    assert_eq!(
        (failed.websocket_failures, failed.websocket_fallback_active),
        (1, Some(true))
    );
    assert!(active(Some("snapshot")));
    assert_eq!(fell_back.websocket_failures, 0);
    (fell_back, failed)
}

/// Each failure counts and replaces the last error text.
fn failures_repeat(failed: &Stats) -> Stats {
    let error = thrown(Some("Error"), "boom");
    record_web_socket_failure(Some("snapshot"), DiagnosticInput::Error(&error));
    assert_eq!(
        stats("snapshot").last_web_socket_error.as_deref(),
        Some("boom")
    );
    record_web_socket_failure(Some("snapshot"), DiagnosticInput::Text("thrown text"));
    let repeated = stats("snapshot");
    assert_eq!(repeated.websocket_failures, 3);
    assert_eq!(
        repeated.last_web_socket_error.as_deref(),
        Some("thrown text")
    );
    assert_eq!(failed.websocket_failures, 1);
    repeated
}

#[test]
fn maestro_response_sessions_snapshot_socket_debug_state() {
    let _isolated = super::exclusive();
    reset(None);
    assert!(stats_of("snapshot").is_none());
    let (_, failed) = fallback_then_failure();
    let repeated = failures_repeat(&failed);
    assert!(stats_of("snapshot-other").is_none() && !active(Some("snapshot-other")));
    reset(Some("snapshot"));
    assert!(stats_of("snapshot").is_none() && !active(Some("snapshot")));
    assert_eq!(
        repeated.websocket_failures, 3,
        "a snapshot survives the reset"
    );
}

/// Complete one request of the conversation and return the peer's socket.
async fn first_request(chat: &Conversation, listener: &TcpListener) -> Peer {
    let server = async {
        let mut socket = accept(listener).await;
        answer(&mut socket, "r1", "first").await;
        socket
    };
    let (socket, outcome) = join(server, chat.send()).await;
    outcome.result.unwrap();
    socket
}

/// The close frame the peer reads after an explicit close.
const DEBUG_CLOSE: (u16, &str) = (1000, "debug_close");

/// Reset removes one session's counters and fallback state and leaves its connection usable;
/// close keeps the counters.
async fn reset_then_close_one_session(listener: &TcpListener, url: &str) -> Peer {
    let [a, b] = ["ctl-a", "ctl-b"];
    let (chat_a, chat_b) = (
        Conversation::new(url, Some(a), None).await,
        Conversation::new(url, Some(b), None).await,
    );
    let mut peer_a = first_request(&chat_a, listener).await;
    let peer_b = first_request(&chat_b, listener).await;
    for session in [a, b] {
        record_web_socket_failure(Some(session), DiagnosticInput::Text("x"));
    }

    reset(Some(a));
    assert!(stats_of(a).is_none() && !active(Some(a)));
    assert!(stats_of(b).is_some() && active(Some(b)));
    let (request, outcome) = join(answer(&mut peer_a, "r2", "second"), chat_a.send()).await;
    outcome.result.unwrap();
    assert!(request.get("previous_response_id").is_none());
    let fresh = stats(a);
    assert_eq!(
        (
            fresh.requests,
            fresh.connections_created,
            fresh.connections_reused
        ),
        (1, 0, 1)
    );

    record_web_socket_sse_fallback(Some(a));
    let before = stats(a);
    close(Some(a));
    let released = until_released(&mut peer_a).await.unwrap();
    assert_eq!((released.0, released.1.as_str()), DEBUG_CLOSE);
    assert_eq!(
        stats(a),
        before,
        "close keeps the counters and fallback state"
    );
    assert!(cached_entry(a).is_none() && cached_entry(b).is_some());
    peer_b
}

/// Reset of every session leaves connections; close of every session leaves counters.
async fn reset_all_then_close_all(mut peer_b: Peer) {
    reset(None);
    assert!(stats_of("ctl-a").is_none() && stats_of("ctl-b").is_none() && !active(Some("ctl-b")));
    acknowledged(&mut peer_b).await;
    assert!(cached_entry("ctl-b").is_some(), "reset leaves connections");
    close(Some(""));
    let released = until_released(&mut peer_b).await.unwrap();
    assert_eq!((released.0, released.1.as_str()), DEBUG_CLOSE);
    assert!(cached_entry("ctl-b").is_none());
}

/// A reset between counting and completion removes the counters for good; the next request
/// creates fresh ones.
async fn reset_while_in_flight(listener: &TcpListener, url: &str) {
    let session = "ctl-c";
    let chat = Conversation::new(url, Some(session), None).await;
    let mut peer = first_request(&chat, listener).await;
    let (counted_tx, counted_rx) = oneshot::channel::<()>();
    let (go_tx, go_rx) = oneshot::channel::<()>();
    let server = async {
        read_request(&mut peer).await;
        counted_tx.send(()).unwrap();
        go_rx.await.unwrap();
        respond(&mut peer, Some("r2"), "second").await;
    };
    let controller = async {
        counted_rx.await.unwrap();
        assert_eq!(stats(session).requests, 2);
        reset(Some(session));
        assert!(stats_of(session).is_none());
        go_tx.send(()).unwrap();
    };
    let ((), (), second) = join3(server, controller, chat.send()).await;
    second.result.unwrap();
    assert!(
        stats_of(session).is_none(),
        "completion does not recreate the counters"
    );
    let (_, third) = join(answer(&mut peer, "r3", "third"), chat.send()).await;
    third.result.unwrap();
    let next = stats(session);
    assert_eq!(
        (
            next.requests,
            next.connections_created,
            next.connections_reused
        ),
        (1, 0, 1)
    );
}

#[test]
fn maestro_response_sessions_separate_socket_reset_and_close() {
    let _isolated = super::exclusive();
    run_native(async {
        let (listener, url) = listen().await;
        let peer_b = reset_then_close_one_session(&listener, &url).await;
        reset_all_then_close_all(peer_b).await;
        reset_while_in_flight(&listener, &url).await;
        record_web_socket_sse_fallback(Some("ctl-fresh"));
        assert!(
            cached_entry("ctl-fresh").is_none(),
            "recording opens no connection"
        );
        close(None);
        reset(None);
    });
}
