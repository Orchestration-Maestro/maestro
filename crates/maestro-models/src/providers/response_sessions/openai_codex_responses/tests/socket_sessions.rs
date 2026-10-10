//! Cached socket lifetime and ownership witnesses against real loopback peers.

use super::super::continuation::{Continuation, select};
use super::super::debug::{
    get_openai_codex_web_socket_debug_stats as stats_of, is_web_socket_sse_fallback_active,
    record_web_socket_failure, record_web_socket_sse_fallback,
    reset_openai_codex_web_socket_debug_stats as reset,
};
use super::super::headers::build_web_socket_headers;
use super::super::sessions::{
    Claim, CloseReason, Identity, Lease, Socket, acquire, cached_entry, claim,
    close_openai_codex_web_socket_sessions as close, lock, publish,
};
use super::super::websocket::{
    WebSocketOutput, process_web_socket_stream, resolve_codex_web_socket_url,
};
use super::super::{CodexError, OpenAICodexResponsesOptions};
use super::loopback::{Conversation, answer, read_request, respond};
use super::socket_transport::{
    Outcome, accept, acknowledged, closing, listen, opening, run_native, text_of, until_released,
};
use super::sockets::block_on;
use crate::{Cancellation, DiagnosticInput, Transport};
use base64::Engine as _;
use futures_util::future::{Either, join, join3, select as select_first};
use futures_util::{FutureExt, SinkExt};
use indexmap::IndexMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::AsyncWriteExt as _;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use tokio::time::advance;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::handshake::server::{
    Callback, ErrorResponse, Request, Response,
};
use tokio_tungstenite::tungstenite::http::HeaderMap;
use tokio_tungstenite::tungstenite::protocol::{CloseFrame, frame::coding::CloseCode};
use tokio_tungstenite::{WebSocketStream, accept_hdr_async, connect_async};

/// The peer's end of a socket.
type Peer = WebSocketStream<TcpStream>;

/// Cached transport, which retains the context of each completed request.
const CACHED: Option<Transport> = Some(Transport::WebsocketCached);

/// A socket the cache could own, and the peer it talks to.
async fn pair() -> (Socket, Peer) {
    let (listener, url) = listen().await;
    let (client, server) = join(connect_async(url.replace("http", "ws")), accept(&listener)).await;
    (client.unwrap().0, server)
}

/// Two such pairs.
async fn pairs() -> ((Socket, Peer), (Socket, Peer)) {
    join(Box::pin(pair()), Box::pin(pair())).await
}

/// The identity a conversation's requests carry.
fn identity_of(chat: &Conversation) -> Identity {
    let headers = build_web_socket_headers(&chat.setup.prepared.headers, "req").unwrap();
    let url = resolve_codex_web_socket_url(&chat.setup.model.base_url).unwrap();
    Identity::new(&url, &headers)
}

/// An identity that only its endpoint text distinguishes.
fn endpoint(name: &str) -> Identity {
    Identity::new(name, &IndexMap::new())
}

/// Send the conversation's next request and answer it, returning the peer's socket afterwards.
async fn completed(chat: &Conversation, listener: &TcpListener) -> (Peer, Outcome) {
    let server = async {
        let mut socket = accept(listener).await;
        answer(&mut socket, "r1", "first").await;
        socket
    };
    let (socket, outcome) = join(server, chat.send()).await;
    outcome.result.as_ref().unwrap();
    (socket, outcome)
}

/// The close frame we sent, as the peer reads it.
fn closed(reason: &str) -> (u16, String) {
    (1000, reason.to_owned())
}

/// Read until the peer receives our close frame.
async fn close_seen(peer: &mut Peer) -> (u16, String) {
    until_released(peer).await.expect("a close frame")
}

/// Send the events one by one.
async fn send_events(peer: &mut Peer, events: Vec<String>) {
    for event in events {
        peer.send(Message::text(event)).await.unwrap();
    }
}

/// Seen from outside: the failure category and its message.
fn described(result: &Result<(), CodexError>) -> (&'static str, String) {
    match result {
        Ok(()) => ("ok", String::new()),
        Err(CodexError::Api(info)) => ("api", info.message.clone()),
        Err(CodexError::Protocol(info)) => ("protocol", info.message.clone()),
        Err(CodexError::Transport(info)) => ("transport", info.message.clone()),
    }
}

/// Time is paused for the idle-expiry witnesses, and none of them waits on a peer while an idle
/// timer is pending, because the paused clock would run ahead of the I/O.
const fn secs(seconds: u64) -> Duration {
    Duration::from_secs(seconds)
}

/// A session whose socket was released 299999 milliseconds ago is still cached; one millisecond
/// later its owner closes it with the expiry reason.
async fn expires_at_the_deadline(listener: &TcpListener, url: &str) {
    let chat = Conversation::new(url, Some("expire-edge"), CACHED).await;
    let (mut peer, _) = completed(&chat, listener).await;
    advance(Duration::from_millis(299_999)).await;
    assert!(cached_entry("expire-edge").is_some());
    advance(Duration::from_millis(1)).await;
    assert_eq!(close_seen(&mut peer).await, closed("idle_timeout"));
    assert!(cached_entry("expire-edge").is_none());
}

/// A request that arrives before the deadline outlives it, and the next release starts a new
/// full period.
async fn restarts_the_idle_period_at_release(listener: &TcpListener, url: &str) {
    let mut chat = Conversation::new(url, Some("expire-renew"), CACHED).await;
    let (mut peer, first) = completed(&chat, listener).await;
    chat.follow(&first.output, "next");
    advance(secs(100)).await;
    let held = async {
        read_request(&mut peer).await;
        send_events(&mut peer, opening("held")).await;
        advance(secs(250)).await;
        assert!(
            cached_entry("expire-renew").is_some(),
            "busy work outlives the old deadline"
        );
        send_events(&mut peer, closing("held")).await;
    };
    let (second, ()) = join(chat.send(), held).await;
    second.result.unwrap();
    advance(Duration::from_millis(299_999)).await;
    assert!(
        cached_entry("expire-renew").is_some(),
        "the idle period restarts at release"
    );
    advance(Duration::from_millis(1)).await;
    assert_eq!(close_seen(&mut peer).await, closed("idle_timeout"));
}

/// An expiry that fires after an operation claimed the socket leaves it to that operation.
async fn yields_expiry_to_a_claimed_handoff(listener: &TcpListener, url: &str) {
    let chat = Conversation::new(url, Some("expire-claimed"), CACHED).await;
    let (mut peer, _) = completed(&chat, listener).await;
    let Claim::Idle(held) = claim("expire-claimed", &identity_of(&chat)) else {
        panic!("an idle entry for the same identity");
    };
    advance(secs(300)).await;
    assert!(cached_entry("expire-claimed").is_some());
    assert!(held.reused, "a claimed handoff wins over expiry");
    drop(held);
    assert_eq!(close_seen(&mut peer).await, closed("done"));
    close(Some("expire-claimed"));
}

#[test]
fn maestro_response_sessions_expire_idle_sockets() {
    let _isolated = super::exclusive();
    block_on(async {
        let (listener, url) = listen().await;
        expires_at_the_deadline(&listener, &url).await;
        restarts_the_idle_period_at_release(&listener, &url).await;
        yields_expiry_to_a_claimed_handoff(&listener, &url).await;
    });
}

/// A credential whose claim names `account`.
fn token(account: &str, signature: &str) -> String {
    let claim =
        format!(r#"{{"https://api.openai.com/auth":{{"chatgpt_account_id":"{account}"}}}}"#);
    let payload = base64::engine::general_purpose::STANDARD.encode(claim);
    format!("a.{payload}.{signature}")
}

/// Records the headers of the upgrade request it sees.
struct Recorder(Arc<Mutex<Option<HeaderMap>>>);

impl Callback for Recorder {
    fn on_request(self, request: &Request, response: Response) -> Result<Response, ErrorResponse> {
        *self.0.lock().unwrap() = Some(request.headers().clone());
        Ok(response)
    }
}

/// Accept one socket and keep the headers of its upgrade request.
async fn accept_headers(listener: &TcpListener) -> (Peer, HeaderMap) {
    let (stream, _) = listener.accept().await.unwrap();
    let seen = Arc::new(Mutex::new(None));
    let socket = accept_hdr_async(stream, Recorder(Arc::clone(&seen)))
        .await
        .unwrap();
    let headers = seen.lock().unwrap().take().unwrap();
    (socket, headers)
}

/// Authored headers.
fn headers(pairs: &[(&str, &str)]) -> IndexMap<String, String> {
    pairs
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect()
}

/// Start options: one credential and one custom field.
fn base_options(options: &mut OpenAICodexResponsesOptions) {
    options.common.api_key = Some(token("acc_test", "b"));
    options.common.headers = Some(headers(&[("X-Custom", "a")]));
}

/// A conversation on `url` with the start options, changed by `change`.
async fn with_options(
    url: &str,
    session: &str,
    change: impl FnOnce(&mut OpenAICodexResponsesOptions),
) -> Conversation {
    let mut chat = Conversation::new(url, Some(session), None).await;
    base_options(&mut chat.setup.options);
    change(&mut chat.setup.options);
    chat
}

/// Header sets that are equal after name lower-casing and value trimming reuse the connection.
async fn equivalent_headers_reuse(listener: &TcpListener, url: &str) {
    let session = "identity-reuse";
    let first = with_options(url, session, |o| {
        o.common.headers = Some(headers(&[("X-A", "1"), ("x-b", " 2 ")]));
    })
    .await;
    let again = with_options(url, session, |o| {
        o.common.headers = Some(headers(&[("x-b", "2"), ("X-a", "1")]));
    })
    .await;
    let server = async {
        let mut socket = accept(listener).await;
        answer(&mut socket, "r1", "one").await;
        answer(&mut socket, "r2", "two").await;
    };
    let client = async {
        first.send().await.result.unwrap();
        again.send().await.result.unwrap();
    };
    join(server, client).await;
    close(Some(session));
}

/// A field of the conversation's options.
type Change = fn(&mut OpenAICodexResponsesOptions);

/// Changing the credential, the account or a custom field replaces the idle socket, and the
/// new request reaches the endpoint with the new value.
async fn changed_headers_replace_the_socket(listener: &TcpListener, url: &str) {
    let changes: [(&str, Change, &str, String); 3] = [
        (
            "token",
            |o| o.common.api_key = Some(token("acc_test", "c")),
            "authorization",
            format!("Bearer {}", token("acc_test", "c")),
        ),
        (
            "account",
            |o| o.common.api_key = Some(token("acc_other", "b")),
            "chatgpt-account-id",
            "acc_other".to_owned(),
        ),
        (
            "custom",
            |o| o.common.headers = Some(headers(&[("X-Custom", "b")])),
            "x-custom",
            "b".to_owned(),
        ),
    ];
    for (name, change, field, expected) in changes {
        let session = format!("identity-{name}");
        let old = with_options(url, &session, |_| {}).await;
        let new = with_options(url, &session, change).await;
        let server = async {
            let (mut stale, _) = accept_headers(listener).await;
            answer(&mut stale, "r1", "old").await;
            let (mut fresh, seen) = accept_headers(listener).await;
            answer(&mut fresh, "r2", "new").await;
            (close_seen(&mut stale).await, seen)
        };
        let client = async {
            old.send().await.result.unwrap();
            new.send().await.result.unwrap();
        };
        let ((released, seen), ()) = join(server, client).await;
        assert_eq!(released, closed("done"), "{name}");
        assert_eq!(seen[field].to_str().unwrap(), expected, "{name}");
        close(Some(&session));
    }
}

/// Another endpoint replaces the idle socket: the new request arrives there, and the old
/// endpoint sees its socket released instead of a second request.
async fn another_endpoint_replaces_the_socket(listener: &TcpListener, url: &str) {
    let other = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url_other = format!("http://{}", other.local_addr().unwrap());
    let old = Conversation::new(url, Some("identity-url"), None).await;
    let new = Conversation::new(&url_other, Some("identity-url"), None).await;
    let server_old = async {
        let mut socket = accept(listener).await;
        answer(&mut socket, "r1", "old").await;
        close_seen(&mut socket).await
    };
    let server_new = async {
        let mut socket = accept(&other).await;
        answer(&mut socket, "r2", "new").await
    };
    let client = async {
        old.send().await.result.unwrap();
        new.send().await.result.unwrap();
    };
    let (released, request, ()) = join3(server_old, server_new, client).await;
    assert_eq!(released, closed("done"));
    assert_eq!(request["type"], "response.create");
    close(Some("identity-url"));
}

/// The server side of the busy-mismatch request: the owner's held response, then the uncached
/// connection, then the other session's.
async fn busy_servers(
    listener: &TcpListener,
    inflight: oneshot::Sender<()>,
    finish: oneshot::Receiver<()>,
) -> ((u16, String), Peer) {
    let mut socket = accept(listener).await;
    read_request(&mut socket).await;
    send_events(&mut socket, opening("held")).await;
    inflight.send(()).unwrap();
    let mut uncached = accept(listener).await;
    answer(&mut uncached, "r2", "uncached").await;
    let released = close_seen(&mut uncached).await;
    let mut separate = accept(listener).await;
    answer(&mut separate, "r3", "other").await;
    finish.await.unwrap();
    send_events(&mut socket, closing("held")).await;
    (released, separate)
}

/// A busy session serves a request with another identity on a separate uncached connection, and
/// another session never shares either.
async fn busy_mismatch_and_other_session(listener: &TcpListener, url: &str) {
    let owner = with_options(url, "identity-busy", |_| {}).await;
    let changed = with_options(url, "identity-busy", |o| {
        o.common.headers = Some(headers(&[("X-Custom", "b")]));
    })
    .await;
    let other = Conversation::new(url, Some("identity-other"), None).await;
    let (inflight_tx, inflight_rx) = oneshot::channel();
    let (finish_tx, finish_rx) = oneshot::channel();
    let others = async {
        inflight_rx.await.unwrap();
        let entry = cached_entry("identity-busy").unwrap();
        changed.send().await.result.unwrap();
        other.send().await.result.unwrap();
        finish_tx.send(()).unwrap();
        entry
    };
    let server = busy_servers(listener, inflight_tx, finish_rx);
    let ((released, mut separate), (held, entry)) = join(server, join(owner.send(), others)).await;
    held.result.unwrap();
    assert_eq!(released, closed("done"));
    let current = cached_entry("identity-busy").unwrap();
    assert!(Arc::ptr_eq(&current, &entry), "the busy entry stays");
    assert!(cached_entry("identity-other").is_some());
    acknowledged(&mut separate).await;
    close(None);
}

#[test]
fn maestro_response_sessions_bind_socket_identity() {
    let _isolated = super::exclusive();
    run_native(async {
        let (socket, mut peer) = pair().await;
        publish("identity-boundary", endpoint("a"), socket).keep();
        assert!(
            matches!(claim("identity-boundary", &endpoint("b")), Claim::Absent),
            "a changed endpoint cannot claim the earlier socket"
        );
        assert_eq!(close_seen(&mut peer).await, closed("done"));
        let (listener, url) = listen().await;
        equivalent_headers_reuse(&listener, &url).await;
        changed_headers_replace_the_socket(&listener, &url).await;
        another_endpoint_replaces_the_socket(&listener, &url).await;
        busy_mismatch_and_other_session(&listener, &url).await;
    });
}

/// The idle owner of a replaced entry finishes after the replacement is published and must not
/// remove it; the entry, its slot and its state are three different things.
async fn replaced_owner_leaves_the_replacement() -> (Peer, Lease) {
    let session = "replacement";
    let ((old_socket, mut old_peer), (new_socket, new_peer)) = pairs().await;
    publish(session, endpoint("a"), old_socket).keep();
    let old = cached_entry(session).unwrap();
    *lock(&old.continuation) = Continuation::new(&serde_json::json!({}), "old", Vec::new());
    assert!(matches!(claim(session, &endpoint("b")), Claim::Absent));
    let lease = publish(session, endpoint("b"), new_socket);
    let new = cached_entry(session).unwrap();
    assert_eq!(close_seen(&mut old_peer).await, closed("done"));
    assert!(!Arc::ptr_eq(&old, &new), "entry identity");
    assert!(
        Arc::ptr_eq(&cached_entry(session).unwrap(), &new),
        "container slot"
    );
    assert!(
        lock(&old.continuation).is_some() && lock(&new.continuation).is_none(),
        "state"
    );
    let weak = Arc::downgrade(&old);
    drop(old);
    assert!(
        weak.upgrade().is_none(),
        "the last alias releases the entry"
    );
    (new_peer, lease)
}

/// A replaced entry's lease, dropped after the replacement exists, leaves the replacement alone.
async fn replaced_lease_leaves_the_replacement(mut new_peer: Peer, lease: Lease) {
    let session = "replacement";
    close(Some(session));
    let (third_socket, mut third_peer) = pair().await;
    let latest = publish(session, endpoint("c"), third_socket);
    let current = cached_entry(session).unwrap();
    drop(lease);
    assert_eq!(close_seen(&mut new_peer).await, closed("debug_close"));
    assert!(
        Arc::ptr_eq(&cached_entry(session).unwrap(), &current),
        "stale lease drop"
    );
    let stale = publish("replacement-other", endpoint("d"), pair().await.0);
    stale.keep();
    close(Some("replacement-other"));
    latest.keep();
    assert!(
        Arc::ptr_eq(&cached_entry(session).unwrap(), &current),
        "kept lease"
    );
    close(Some(session));
    assert_eq!(close_seen(&mut third_peer).await, closed("debug_close"));
}

/// Two connections finish while the session is empty; whichever finishes first is cached and
/// the other runs uncached and closes after its request.
async fn publish_race(first_gate: usize) {
    let session = format!("race-{first_gate}");
    let ((socket_a, mut peer_a), (socket_b, mut peer_b)) = pairs().await;
    let (open_a, gate_a) = oneshot::channel::<()>();
    let (open_b, gate_b) = oneshot::channel::<()>();
    let (done_a, published_a) = oneshot::channel::<()>();
    let (done_b, published_b) = oneshot::channel::<()>();
    let acquire_a = async {
        let lease = acquire(Some(&session), endpoint("race"), async move {
            gate_a.await.unwrap();
            Ok(socket_a)
        })
        .await;
        done_a.send(()).unwrap();
        lease
    };
    let acquire_b = async {
        let lease = acquire(Some(&session), endpoint("race"), async move {
            gate_b.await.unwrap();
            Ok(socket_b)
        })
        .await;
        done_b.send(()).unwrap();
        lease
    };
    let (first, published, second) = if first_gate == 0 {
        (open_a, published_a, open_b)
    } else {
        (open_b, published_b, open_a)
    };
    let driver = async {
        first.send(()).unwrap();
        published.await.unwrap();
        second.send(()).unwrap();
    };
    let (lease_a, lease_b, ()) = join3(acquire_a, acquire_b, driver).await;
    let (lease_a, lease_b) = (lease_a.unwrap(), lease_b.unwrap());
    let (cached_lease, uncached_lease, uncached_peer) = if first_gate == 0 {
        (lease_a, lease_b, &mut peer_b)
    } else {
        (lease_b, lease_a, &mut peer_a)
    };
    assert!(cached_lease.continuation().is_some());
    assert!(uncached_lease.continuation().is_none());
    uncached_lease.keep();
    assert_eq!(close_seen(uncached_peer).await, closed("done"));
    cached_lease.keep();
    assert!(cached_entry(&session).is_some());
    close(Some(&session));
}

#[test]
fn maestro_response_sessions_keep_replacement_cache_entry() {
    let _isolated = super::exclusive();
    run_native(async {
        let (new_peer, lease) = replaced_owner_leaves_the_replacement().await;
        Box::pin(replaced_lease_leaves_the_replacement(new_peer, lease)).await;
        Box::pin(publish_race(0)).await;
        Box::pin(publish_race(1)).await;
    });
}

/// The owner's server: it holds its response while the other requests run, then answers a
/// request that continues the first.
async fn owner_server(
    listener: &TcpListener,
    inflight: oneshot::Sender<()>,
    finish: oneshot::Receiver<()>,
) -> serde_json::Value {
    let mut socket = accept(listener).await;
    read_request(&mut socket).await;
    send_events(&mut socket, opening("held")).await;
    inflight.send(()).unwrap();
    finish.await.unwrap();
    send_events(&mut socket, closing("held")).await;
    answer(&mut socket, "r4", "later").await
}

/// The server for the two requests that run uncached: one succeeds, one fails.
async fn uncached_servers(
    listener: &TcpListener,
) -> Vec<(serde_json::Value, Option<(u16, String)>)> {
    let mut seen = Vec::new();
    for failing in [false, true] {
        let mut socket = accept(listener).await;
        let request = read_request(&mut socket).await;
        if failing {
            let error = serde_json::json!({"type": "error", "message": "no"}).to_string();
            socket.send(Message::text(error)).await.unwrap();
        } else {
            respond(&mut socket, Some("r2"), "uncached").await;
        }
        seen.push((request, until_released(&mut socket).await));
    }
    seen
}

#[test]
fn maestro_response_sessions_run_busy_session_uncached() {
    let _isolated = super::exclusive();
    run_native(async {
        let (listener, url) = listen().await;
        let session = "busy";
        let mut owner = Conversation::new(&url, Some(session), CACHED).await;
        let second = Conversation::new(&url, Some(session), CACHED).await;
        let third = Conversation::new(&url, Some(session), CACHED).await;
        let (inflight_tx, inflight_rx) = oneshot::channel();
        let (finish_tx, finish_rx) = oneshot::channel();
        let others = async {
            inflight_rx.await.unwrap();
            let entry = cached_entry(session).unwrap();
            let finished = second.send().await;
            let failed = third.send().await;
            finish_tx.send(()).unwrap();
            (entry, finished, failed)
        };
        let client = async {
            let (first, others) = join(owner.send(), others).await;
            owner.follow(&first.output, "next");
            (first, others, owner.send().await)
        };
        let primary = owner_server(&listener, inflight_tx, finish_rx);
        let (later, seen, (first, (entry, finished, failed), next)) =
            Box::pin(join3(primary, uncached_servers(&listener), client)).await;
        first.result.unwrap();
        next.result.unwrap();
        finished.result.unwrap();
        assert!(matches!(failed.result, Err(CodexError::Api(_))));
        for (request, released) in &seen {
            assert!(request.get("previous_response_id").is_none());
            assert_eq!(*released, Some(closed("done")));
        }
        let current = cached_entry(session).unwrap();
        assert!(Arc::ptr_eq(&current, &entry), "the owner keeps the entry");
        assert_eq!(later["previous_response_id"], "r1");
        let counted = stats_of(session).unwrap();
        assert_eq!(
            (counted.connections_created, counted.connections_reused),
            (3, 1)
        );
        close(Some(session));
        reset(Some(session));
    });
}

/// The idle peer ends the connection, and the next request opens a new one with the full input.
async fn replaced_after_peer_ending(ending: &str) {
    let session = format!("closed-{ending}");
    let (listener, url) = listen().await;
    let mut chat = Conversation::new(&url, Some(&session), CACHED).await;
    let (mut peer, first) = completed(&chat, &listener).await;
    chat.follow(&first.output, "next");
    match ending {
        "close" => {
            peer.send(Message::Close(None)).await.unwrap();
            until_released(&mut peer).await;
        }
        "eof" => {
            drop(peer);
            // The idle owner removes its slot as it ends, which follows its read of the end.
            while cached_entry(&session).is_some() {
                tokio::task::yield_now().await;
            }
        }
        _ => {
            peer.get_mut().write_all(&[0xFF, 0x00]).await.unwrap();
            until_released(&mut peer).await;
        }
    }
    let server = async {
        let mut socket = accept(&listener).await;
        answer(&mut socket, "r2", "again").await
    };
    let (request, outcome) = join(server, chat.send()).await;
    outcome.result.unwrap();
    assert!(request.get("previous_response_id").is_none(), "{ending}");
    assert_eq!(request["input"].as_array().unwrap().len(), 3, "{ending}");
    close(Some(&session));
}

/// A ping that arrives while the socket is idle is answered and published to nobody.
async fn idle_ping_publishes_nothing() {
    let (listener, url) = listen().await;
    let chat = Conversation::new(&url, Some("closed-ping"), CACHED).await;
    let (mut peer, first) = completed(&chat, &listener).await;
    while first.events.next().now_or_never().flatten().is_some() {}
    acknowledged(&mut peer).await;
    assert!(
        first.events.next().now_or_never().is_none(),
        "an idle ping publishes nothing"
    );
    close(Some("closed-ping"));
}

#[test]
fn maestro_response_sessions_replace_closed_socket() {
    let _isolated = super::exclusive();
    run_native(async {
        for ending in ["close", "eof", "garbage"] {
            replaced_after_peer_ending(ending).await;
        }
        idle_ping_publishes_nothing().await;
    });
}

/// Serve two requests of one conversation: on one socket when it is cached, otherwise on two
/// sockets that are each released after their request.
async fn serve_two(listener: &TcpListener, cached: bool) -> usize {
    let mut socket = accept(listener).await;
    answer(&mut socket, "r1", "one").await;
    if cached {
        answer(&mut socket, "r2", "two").await;
        return 1;
    }
    assert_eq!(close_seen(&mut socket).await, closed("done"));
    let mut second = accept(listener).await;
    answer(&mut second, "r2", "two").await;
    assert_eq!(close_seen(&mut second).await, closed("done"));
    2
}

/// Whether a session key makes the socket cacheable, and what that counts.
async fn scoped_session(session: Option<&str>, cached: bool) {
    let (listener, url) = listen().await;
    let chat = Conversation::new(&url, session, None).await;
    let client = async {
        chat.send().await.result.unwrap();
        chat.send().await.result.unwrap();
    };
    let (connections, ()) = join(serve_two(&listener, cached), client).await;
    assert_eq!(connections, if cached { 1 } else { 2 }, "{session:?}");
    let counted = session.and_then(stats_of);
    assert_eq!(counted.is_some(), cached, "{session:?}");
    if let Some(counted) = counted {
        assert_eq!(
            (counted.connections_created, counted.connections_reused),
            (1, 1)
        );
    }
    if let Some(session) = session {
        close(Some(session));
        reset(Some(session));
    }
}

/// Two sessions on one endpoint each keep their own socket and counters.
async fn independent_sessions() {
    let (listener, url) = listen().await;
    let first = Conversation::new(&url, Some("scope-x"), None).await;
    let second = Conversation::new(&url, Some("scope-y"), None).await;
    let server = async {
        let mut x = accept(&listener).await;
        answer(&mut x, "x1", "x").await;
        let mut y = accept(&listener).await;
        answer(&mut y, "y1", "y").await;
        answer(&mut x, "x2", "x").await;
        answer(&mut y, "y2", "y").await;
    };
    let client = async {
        for chat in [&first, &second, &first, &second] {
            chat.send().await.result.unwrap();
        }
    };
    join(server, client).await;
    for session in ["scope-x", "scope-y"] {
        let counted = stats_of(session).unwrap();
        assert_eq!(
            (counted.requests, counted.connections_created),
            (2, 1),
            "{session}"
        );
    }
    close(None);
    reset(None);
}

#[test]
fn maestro_response_sessions_scope_missing_socket_session() {
    let _isolated = super::exclusive();
    run_native(async {
        let sessions = [
            (None, false),
            (Some(""), false),
            (Some("scope"), true),
            (Some(" "), true),
        ];
        for (session, cached) in sessions {
            scoped_session(session, cached).await;
        }
        independent_sessions().await;
        let failure = DiagnosticInput::Text("ignored");
        for session in [None, Some("")] {
            record_web_socket_failure(session, failure);
            record_web_socket_sse_fallback(session);
            assert!(!is_web_socket_sse_fallback_active(session));
        }
        assert!(stats_of("").is_none());
    });
}

/// How the second request of a cached conversation ends badly.
#[derive(Clone, Copy)]
enum Fault {
    /// The server reports an error event.
    Api,
    /// The server sends text that is not JSON.
    Protocol,
    /// The reducer rejects an item that lacks its call identifier.
    Reducer,
    /// The server closes the socket.
    PeerClose,
    /// The caller cancels while reading.
    Cancel,
}

/// The faults with the category and message each must report.
const FAULTS: [(Fault, &str, &str); 5] = [
    (Fault::Api, "api", "Codex error: no"),
    (Fault::Protocol, "protocol", "Invalid Codex WebSocket JSON"),
    (
        Fault::Reducer,
        "protocol",
        "invalid type: null, expected a string",
    ),
    (Fault::PeerClose, "transport", "WebSocket closed 1011 oops"),
    (Fault::Cancel, "transport", "Request was aborted"),
];

/// Play the server side of the second request: partial output, then the fault.
async fn misbehave(peer: &mut Peer, fault: Fault, partial: oneshot::Sender<()>) {
    read_request(peer).await;
    send_events(peer, opening("partial")).await;
    let event = match fault {
        Fault::Api => serde_json::json!({"type": "error", "message": "no"}).to_string(),
        Fault::Protocol => "{bad}".to_owned(),
        Fault::Reducer => {
            serde_json::json!({"type": "response.output_item.added", "item": {"type": "function_call"}})
                .to_string()
        }
        Fault::PeerClose => {
            let frame = CloseFrame {
                code: CloseCode::Error,
                reason: "oops".into(),
            };
            peer.send(Message::Close(Some(frame))).await.unwrap();
            return;
        }
        Fault::Cancel => {
            acknowledged(peer).await;
            partial.send(()).unwrap();
            return;
        }
    };
    peer.send(Message::text(event)).await.unwrap();
}

/// The server for a faulty second request, then for the fresh connection that follows.
async fn faulty_server(
    listener: &TcpListener,
    fault: Fault,
    partial: oneshot::Sender<()>,
) -> (Option<(u16, String)>, serde_json::Value) {
    let mut first = accept(listener).await;
    answer(&mut first, "r1", "first").await;
    misbehave(&mut first, fault, partial).await;
    let released = until_released(&mut first).await;
    let mut fresh = accept(listener).await;
    (released, answer(&mut fresh, "r3", "third").await)
}

/// One fault: the failure keeps its category and message, the earlier output stays, the socket
/// leaves the cache and the context is cleared, and nothing counts as a fallback.
async fn failed_request(index: usize, fault: Fault, category: &str, message: &str) {
    let session = format!("failed-{index}");
    let (listener, url) = listen().await;
    let signal = Cancellation::new();
    let mut chat = Conversation::new(&url, Some(&session), CACHED).await;
    chat.setup.options.common.signal = Some(signal.clone());
    let (partial_tx, partial_rx) = oneshot::channel();
    let cancelling = async {
        if matches!(fault, Fault::Cancel) {
            partial_rx.await.unwrap();
            signal.abort();
        }
    };
    let client = async {
        let first = chat.send().await;
        chat.follow(&first.output, "u2");
        let entry = cached_entry(&session).unwrap();
        let (second, ()) = join(chat.send(), cancelling).await;
        assert!(cached_entry(&session).is_none(), "{category}");
        chat.setup.options.common.signal = None;
        chat.follow(&second.output, "u3");
        (entry, second, chat.send().await)
    };
    let server = faulty_server(&listener, fault, partial_tx);
    let ((released, third_request), (entry, second, third)) = join(server, client).await;
    let (found, text) = described(&second.result);
    assert_eq!(
        (found, text.starts_with(message)),
        (category, true),
        "{text}"
    );
    assert_eq!(text_of(&second.output), "partial");
    assert!(lock(&entry.continuation).is_none(), "{category}");
    third.result.unwrap();
    assert!(third_request.get("previous_response_id").is_none());
    let expected = (!matches!(fault, Fault::PeerClose)).then_some(closed("done"));
    assert_eq!(released, expected, "{category}");
    let counted = stats_of(&session).unwrap();
    let outer = (
        counted.websocket_failures,
        counted.sse_fallbacks,
        counted.websocket_fallback_active,
    );
    assert_eq!(outer, (0, 0, None));
    close(Some(&session));
    reset(Some(&session));
}

#[test]
fn maestro_response_sessions_clear_failed_socket_continuation() {
    let _isolated = super::exclusive();
    run_native(async {
        for (index, (fault, category, message)) in FAULTS.into_iter().enumerate() {
            Box::pin(failed_request(index, fault, category, message)).await;
        }
    });
}

/// A synchronously claimed socket is released when its acquisition result is dropped.
async fn dropped_handoff(listener: &TcpListener, url: &str) {
    let chat = Conversation::new(url, Some("drop-handoff"), CACHED).await;
    let (mut peer, _) = completed(&chat, listener).await;
    let pending = acquire(
        Some("drop-handoff"),
        identity_of(&chat),
        std::future::pending(),
    );
    let held = pending
        .now_or_never()
        .expect("synchronous ownership")
        .unwrap();
    assert!(held.reused);
    drop(held);
    assert!(
        cached_entry("drop-handoff").is_none(),
        "no permanently busy slot"
    );
    assert_eq!(close_seen(&mut peer).await, closed("done"));
}

/// Drop a request when the peer reports `ready`.
async fn drop_when(ready: oneshot::Receiver<()>, request: Outcomeless<'_>) {
    match select_first(request, ready).await {
        Either::Right((signalled, request)) => {
            signalled.unwrap();
            drop(request);
        }
        Either::Left(_) => panic!("finished before the drop"),
    }
}

/// A boxed request future.
type Outcomeless<'a> = std::pin::Pin<Box<dyn std::future::Future<Output = Outcome> + 'a>>;

/// A request future dropped while it reads a response.
async fn dropped_active_request(listener: &TcpListener, url: &str) {
    let chat = Conversation::new(url, Some("drop-active"), CACHED).await;
    let (ready_tx, ready_rx) = oneshot::channel::<()>();
    let server = async {
        let mut socket = accept(listener).await;
        read_request(&mut socket).await;
        send_events(&mut socket, opening("pending")).await;
        acknowledged(&mut socket).await;
        ready_tx.send(()).unwrap();
        until_released(&mut socket).await;
    };
    join(server, drop_when(ready_rx, Box::pin(chat.send()))).await;
    assert!(cached_entry("drop-active").is_none());
}

/// A request future dropped while its upgrade is still pending.
async fn dropped_connect(listener: &TcpListener, url: &str) {
    let chat = Conversation::new(url, Some("drop-connect"), CACHED).await;
    let (accepted_tx, accepted_rx) = oneshot::channel::<()>();
    let server = async {
        let (stream, _) = listener.accept().await.unwrap();
        accepted_tx.send(()).unwrap();
        super::socket_transport::drain(&stream).await;
    };
    join(server, drop_when(accepted_rx, Box::pin(chat.send()))).await;
    assert!(cached_entry("drop-connect").is_none());
}

/// The final claimed owner releases its socket despite a surviving entry alias.
async fn dropped_cache_owner(listener: &TcpListener, url: &str) {
    let chat = Conversation::new(url, Some("drop-owner"), CACHED).await;
    let (mut peer, _) = completed(&chat, listener).await;
    let entry = cached_entry("drop-owner").unwrap();
    let Claim::Idle(held) = claim("drop-owner", &identity_of(&chat)) else {
        panic!("an idle entry for the same identity");
    };
    drop(held);
    assert_eq!(close_seen(&mut peer).await, closed("done"));
    drop(entry);
    close(Some("drop-owner"));
}

#[test]
fn maestro_response_sessions_release_socket_handoff_on_drop() {
    let _isolated = super::exclusive();
    run_native(async {
        let (listener, url) = listen().await;
        dropped_handoff(&listener, &url).await;
        dropped_active_request(&listener, &url).await;
        dropped_connect(&listener, &url).await;
        dropped_cache_owner(&listener, &url).await;
    });
}

/// The server for the request that an explicit close interrupts.
async fn interrupted_server(listener: &TcpListener, held: oneshot::Sender<()>) -> (u16, String) {
    let mut socket = accept(listener).await;
    read_request(&mut socket).await;
    send_events(&mut socket, opening("partial")).await;
    acknowledged(&mut socket).await;
    held.send(()).unwrap();
    let released = close_seen(&mut socket).await;
    // Answering the close frame is what ends the client's wait.
    socket.flush().await.unwrap();
    released
}

/// The server for the uncached request that runs beside the interrupted one.
async fn side_server(
    listener: &TcpListener,
    started: oneshot::Sender<()>,
    finish: oneshot::Receiver<()>,
) -> (u16, String) {
    let mut socket = accept(listener).await;
    read_request(&mut socket).await;
    send_events(&mut socket, opening("fallback")).await;
    acknowledged(&mut socket).await;
    started.send(()).unwrap();
    finish.await.unwrap();
    send_events(&mut socket, closing("fallback")).await;
    close_seen(&mut socket).await
}

/// One close control while a request is active, an uncached request runs beside it and another
/// session is idle.
async fn close_during_use(control: Option<&str>, listener: &TcpListener, url: &str) {
    let active = Conversation::new(url, Some("close-active"), CACHED).await;
    let side = Conversation::new(url, Some("close-active"), CACHED).await;
    let idle = Conversation::new(url, Some("close-idle"), CACHED).await;
    let (idle_peer, _) = completed(&idle, listener).await;
    let (held_tx, held_rx) = oneshot::channel::<()>();
    let (started_tx, started_rx) = oneshot::channel::<()>();
    let (finish_tx, finish_rx) = oneshot::channel::<()>();
    let closer = async {
        started_rx.await.unwrap();
        close(control);
        assert!(cached_entry("close-active").is_none());
        tokio::task::yield_now().await;
        finish_tx.send(()).unwrap();
    };
    let beside = async {
        held_rx.await.unwrap();
        join(side.send(), closer).await.0
    };
    let servers = join(
        interrupted_server(listener, held_tx),
        side_server(listener, started_tx, finish_rx),
    );
    let ((released, side_released), (interrupted, side_outcome)) =
        join(servers, join(active.send(), beside)).await;
    assert_interrupted(&interrupted, &released);
    side_outcome.result.unwrap();
    assert_eq!(side_released, closed("done"));
    after_close(control, idle_peer).await;
}

/// The interrupted request reports the close the peer answered with and keeps its output.
fn assert_interrupted(interrupted: &Outcome, released: &(u16, String)) {
    let (kind, text) = described(&interrupted.result);
    assert_eq!(
        (kind, text.as_str()),
        ("transport", "WebSocket closed 1000 debug_close")
    );
    assert_eq!(text_of(&interrupted.output), "partial");
    assert_eq!(*released, closed("debug_close"));
}

/// Which sessions the close left, and what the idle session's peer saw.
async fn after_close(control: Option<&str>, mut idle_peer: Peer) {
    assert!(
        cached_entry("close-active").is_none(),
        "an old release does not reinsert"
    );
    if control == Some("close-active") {
        assert!(
            cached_entry("close-idle").is_some(),
            "a selected close leaves other sessions"
        );
        close(Some("close-idle"));
    }
    assert_eq!(close_seen(&mut idle_peer).await, closed("debug_close"));
    assert!(cached_entry("close-idle").is_none());
}

/// A connect that is pending at close may still publish when it finishes, and a close request
/// made before anyone listens is still seen.
async fn close_while_connecting() {
    let (socket, mut peer) = pair().await;
    let (gate, gated) = oneshot::channel::<()>();
    let connect = async move {
        gated.await.unwrap();
        Ok(socket)
    };
    let pending = acquire(Some("close-connecting"), endpoint("c"), connect);
    let closer = async {
        close(Some("close-connecting"));
        gate.send(()).unwrap();
    };
    let (lease, ()) = join(pending, closer).await;
    let lease = lease.unwrap();
    assert!(
        cached_entry("close-connecting").is_some(),
        "a connect pending at close may publish"
    );
    close(Some("close-connecting"));
    assert_eq!(
        *lease.closed().unwrap().borrow(),
        Some(CloseReason::DebugClose)
    );
    drop(lease);
    assert_eq!(close_seen(&mut peer).await, closed("debug_close"));
}

#[test]
fn maestro_response_sessions_close_active_socket_session() {
    let _isolated = super::exclusive();
    run_native(async {
        let (listener, url) = listen().await;
        for control in [Some("close-active"), None, Some("")] {
            Box::pin(close_during_use(control, &listener, &url)).await;
        }
        close_while_connecting().await;
    });
}

/// The server for a request that an explicit close interrupts: once the close frame has reached
/// its socket, and before it acknowledges the frame, it sends `late`.
async fn late_frames_server(
    listener: &TcpListener,
    held: oneshot::Sender<()>,
    late: Vec<String>,
) -> (u16, String) {
    let mut socket = accept(listener).await;
    read_request(&mut socket).await;
    send_events(&mut socket, opening("partial")).await;
    acknowledged(&mut socket).await;
    held.send(()).unwrap();
    socket.get_ref().peek(&mut [0_u8; 1]).await.unwrap();
    send_events(&mut socket, late).await;
    let released = close_seen(&mut socket).await;
    socket.flush().await.unwrap();
    released
}

/// Request an explicit close of the session while a request is in flight and the peer still
/// delivers `late` messages.
async fn close_with_late_frames(
    session: &str,
    late: Vec<String>,
    listener: &TcpListener,
    url: &str,
) -> (Outcome, (u16, String)) {
    let chat = Conversation::new(url, Some(session), CACHED).await;
    let (held_tx, held_rx) = oneshot::channel::<()>();
    let closer = async {
        held_rx.await.unwrap();
        close(Some(session));
    };
    let server = late_frames_server(listener, held_tx, late);
    let (released, (outcome, ())) = join(server, join(chat.send(), closer)).await;
    (outcome, released)
}

#[test]
fn maestro_response_sessions_keep_frames_in_flight_at_explicit_close() {
    let _isolated = super::exclusive();
    run_native(async {
        let (listener, url) = listen().await;
        let delta = serde_json::json!({"type":"response.output_text.delta","delta":"more"});
        let error = serde_json::json!({"type":"error","message":"no"});
        let late = vec![delta.to_string(), error.to_string()];
        let (failed, released) = close_with_late_frames("late-error", late, &listener, &url).await;
        assert_eq!(
            described(&failed.result),
            ("api", "Codex error: no".to_owned())
        );
        assert_eq!(text_of(&failed.output), "partialmore");
        assert_eq!(released, closed("debug_close"));

        let late = vec![delta.to_string(), closing("partialmore")[0].clone()];
        let late = [late, closing("")[1..].to_vec()].concat();
        let (finished, released) = close_with_late_frames("late-done", late, &listener, &url).await;
        finished.result.unwrap();
        assert_eq!(text_of(&finished.output), "partialmore");
        assert_eq!(released, closed("debug_close"));
    });
}

#[test]
fn maestro_response_sessions_send_requested_close_reason_when_dropped() {
    let _isolated = super::exclusive();
    run_native(async {
        let (listener, url) = listen().await;
        let chat = Conversation::new(&url, Some("close-drop"), CACHED).await;
        let (ready_tx, ready_rx) = oneshot::channel::<()>();
        let server = async {
            let mut socket = accept(&listener).await;
            read_request(&mut socket).await;
            send_events(&mut socket, opening("partial")).await;
            acknowledged(&mut socket).await;
            ready_tx.send(()).unwrap();
            close_seen(&mut socket).await
        };
        let client = async {
            match select_first(Box::pin(chat.send()), ready_rx).await {
                Either::Right((signalled, request)) => {
                    signalled.unwrap();
                    close(Some("close-drop"));
                    drop(request);
                }
                Either::Left(_) => panic!("finished before the drop"),
            }
        };
        let (released, ()) = join(server, client).await;
        assert_eq!(released, closed("debug_close"));
    });
}

/// Run the conversation's next request, cancelling at the start notification.
async fn send_cancelling_at_start(chat: &Conversation, signal: &Cancellation) -> Outcome {
    let prepared = chat.prepared().await;
    let output = Arc::new(std::sync::RwLock::new(
        crate::providers::assistant_output::initial_message(&chat.setup.model),
    ));
    let events = crate::AssistantMessageEventStream::new();
    let mut on_start = || signal.abort();
    let result = process_web_socket_stream(
        &prepared,
        &chat.setup.model,
        &chat.setup.options,
        WebSocketOutput {
            output: &output,
            stream: &events,
            on_start: &mut on_start,
        },
        "req",
    )
    .await;
    Outcome {
        result,
        output,
        events,
    }
}

#[test]
fn maestro_response_sessions_keep_post_completion_cancellation_state() {
    let _isolated = super::exclusive();
    run_native(async {
        let (listener, url) = listen().await;
        let signal = Cancellation::new();
        let mut chat = Conversation::new(&url, Some("late-abort"), CACHED).await;
        chat.setup.options.common.signal = Some(signal.clone());
        let (mut peer, first) = completed(&chat, &listener).await;
        chat.follow(&first.output, "next");
        let entry = cached_entry("late-abort").unwrap();
        let body = chat.prepared().await.body;
        let terminal = serde_json::json!({"type":"response.completed","response":{"id":"r2","status":"completed"}});
        let server = async {
            read_request(&mut peer).await;
            peer.send(Message::text(terminal.to_string()))
                .await
                .unwrap();
            close_seen(&mut peer).await
        };
        let (released, outcome) = join(server, send_cancelling_at_start(&chat, &signal)).await;
        outcome.result.unwrap();
        let response_id = outcome.output.read().unwrap().response_id.clone();
        assert_eq!(response_id.as_deref(), Some("r2"));
        assert_eq!(released, closed("done"));
        assert!(
            cached_entry("late-abort").is_none(),
            "the socket is released"
        );
        let kept = select(&entry.continuation, &body).expect("the earlier context is eligible");
        assert_eq!(kept.previous_response_id.as_deref(), Some("r1"));
    });
}

#[test]
fn maestro_response_sessions_discard_idle_application_frames() {
    let _isolated = super::exclusive();
    run_native(async {
        let (listener, url) = listen().await;
        let mut chat = Conversation::new(&url, Some("idle-frames"), CACHED).await;
        let (mut peer, first) = completed(&chat, &listener).await;
        while first.events.next().now_or_never().flatten().is_some() {}
        let stray = serde_json::json!({"type": "response.created", "response": {"id": "stray"}});
        let frames = [
            Message::text("{malformed"),
            Message::text(stray.to_string()),
            Message::binary(vec![1, 2, 3]),
        ];
        for frame in frames {
            peer.send(frame).await.unwrap();
        }
        acknowledged(&mut peer).await;
        assert!(
            first.events.next().now_or_never().is_none(),
            "nothing reaches a finished operation"
        );
        chat.follow(&first.output, "next");
        let (request, second) = join(answer(&mut peer, "r2", "second"), chat.send()).await;
        second.result.unwrap();
        assert_eq!(request["previous_response_id"], "r1");
        let response_id = second.output.read().unwrap().response_id.clone();
        assert_eq!(response_id.as_deref(), Some("r2"));
        assert_eq!(text_of(&second.output), "second");
        close(Some("idle-frames"));
    });
}

#[test]
fn maestro_response_sessions_claim_idle_socket_synchronously() {
    let _isolated = super::exclusive();
    run_native(async {
        let (listener, url) = listen().await;
        let chat = Conversation::new(&url, Some("sync-claim"), CACHED).await;
        let (mut peer, _) = completed(&chat, &listener).await;
        let Claim::Idle(lease) = claim("sync-claim", &identity_of(&chat)) else {
            panic!("a matching idle socket is owned synchronously");
        };
        assert!(lease.reused);
        assert!(matches!(
            claim("sync-claim", &identity_of(&chat)),
            Claim::Busy
        ));
        drop(lease);
        assert!(cached_entry("sync-claim").is_none());
        assert_eq!(close_seen(&mut peer).await, closed("done"));
    });
}
