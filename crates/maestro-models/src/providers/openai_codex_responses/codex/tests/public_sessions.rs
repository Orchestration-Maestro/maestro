//! Public session controls and resource cleanup.

use super::super::*;
use crate::{Context, Model, StopReason, Transport};
use futures_util::{StreamExt, future::join};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::net::TcpListener;
use tokio_tungstenite::{WebSocketStream, accept_async};

/// A real retained peer and the caller operands that opened it.
struct Retained {
    /// Peer socket.
    peer: WebSocketStream<tokio::net::TcpStream>,
    /// Caller descriptor.
    model: Model,
    /// Caller history.
    context: Context,
    /// Caller options.
    options: OpenAICodexResponsesOptions,
}

/// Seed one session through the public stream, not through private cache insertion.
async fn retained(session: &str) -> Retained {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (model, context, mut options) = super::invocation();
    let mut model = (*model).clone();
    model.base_url = format!("http://{}", listener.local_addr().unwrap());
    options.common.session_id = Some(session.into());
    options.common.transport = Some(Transport::Auto);
    let stream =
        stream_openai_codex_responses(model.clone(), context.clone(), Some(options.clone()));
    let server = async {
        let mut peer = accept_async(listener.accept().await.unwrap().0)
            .await
            .unwrap();
        super::loopback::answer(&mut peer, "seed", "seed reply").await;
        peer
    };
    let (peer, output) = join(server, stream.result()).await;
    assert_eq!(output.read().unwrap().stop_reason, StopReason::Stop);
    Retained {
        peer,
        model,
        context,
        options,
    }
}

/// A session whose socket dials a loopback listener and whose HTTP-stream fallback counts its
/// requests.
struct Fallback {
    /// Caller descriptor.
    model: Model,
    /// Caller history.
    context: Context,
    /// Caller options.
    options: OpenAICodexResponsesOptions,
    /// HTTP-stream requests served so far.
    http_calls: Arc<AtomicUsize>,
}

impl Fallback {
    /// Bind the session's listener and count its HTTP-stream fallback requests.
    async fn session(session: &str) -> (TcpListener, Self) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let (model, context, mut options) = super::invocation();
        let mut model = (*model).clone();
        model.base_url = format!("http://{}", listener.local_addr().unwrap());
        options.common.session_id = Some(session.into());
        options.common.transport = Some(Transport::Auto);
        let http_calls = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&http_calls);
        options.common.fetch = Some(Arc::new(move |_| {
            count.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Ok(super::public_streams::response()) })
        }));
        (
            listener,
            Self {
                model,
                context,
                options,
                http_calls,
            },
        )
    }

    /// Run one request and report how it ended.
    async fn run(&self) -> StopReason {
        let stream = stream_openai_codex_responses(
            self.model.clone(),
            self.context.clone(),
            Some(self.options.clone()),
        );
        stream.result().await.read().unwrap().stop_reason.clone()
    }

    /// HTTP-stream requests served so far.
    fn http_calls(&self) -> usize {
        self.http_calls.load(Ordering::SeqCst)
    }
}

/// Observe the close frame and consume the socket's remaining traffic.
async fn closed(peer: &mut WebSocketStream<tokio::net::TcpStream>) {
    let mut close = false;
    while let Some(message) = peer.next().await {
        if let Ok(tokio_tungstenite::tungstenite::Message::Close(frame)) = message {
            assert_eq!(frame.unwrap().reason, "debug_close");
            close = true;
        }
    }
    assert!(close);
}

#[test]
fn maestro_response_sessions_sticky_fallback_reset_and_close() {
    let _guard = super::exclusive();
    crate::tests::runtime::run(
        maestro_response_sessions_sticky_fallback_reset_and_close_scenario(),
    );
}

/// Execute the controlled public-operation scenario.
async fn maestro_response_sessions_sticky_fallback_reset_and_close_scenario() {
    reset_openai_codex_web_socket_debug_stats(None);
    super::super::debug::record_web_socket_failure(
        Some("sticky-public"),
        crate::DiagnosticInput::Text("controlled failure"),
    );
    for mode in [
        None,
        Some(Transport::Sse),
        Some(Transport::Websocket),
        Some(Transport::WebsocketCached),
        Some(Transport::Auto),
    ] {
        sticky_mode(mode).await;
    }
    let mut snapshot = get_openai_codex_web_socket_debug_stats("sticky-public").unwrap();
    snapshot.requests = 999;
    assert_ne!(
        get_openai_codex_web_socket_debug_stats("sticky-public")
            .unwrap()
            .requests,
        snapshot.requests
    );
    reset_openai_codex_web_socket_debug_stats(Some("sticky-public"));
    assert!(get_openai_codex_web_socket_debug_stats("sticky-public").is_none());
    let mut kept = retained("sticky-public").await;
    let stream = stream_openai_codex_responses(kept.model, kept.context, Some(kept.options));
    let (_, output) = join(
        super::loopback::answer(&mut kept.peer, "retry", "retried"),
        stream.result(),
    )
    .await;
    assert_eq!(output.read().unwrap().stop_reason, StopReason::Stop);
    let stats = get_openai_codex_web_socket_debug_stats("sticky-public").unwrap();
    assert_eq!(
        (
            stats.requests,
            stats.connections_created,
            stats.connections_reused
        ),
        (2, 1, 1)
    );
    close_openai_codex_web_socket_sessions(Some("sticky-public"));
    closed(&mut kept.peer).await;
    assert_eq!(
        get_openai_codex_web_socket_debug_stats("sticky-public"),
        Some(stats)
    );
}

#[test]
fn maestro_response_sessions_failure_creates_sticky_fallback() {
    let _guard = super::exclusive();
    crate::tests::runtime::run(async {
        reset_openai_codex_web_socket_debug_stats(None);
        let (listener, session) = Fallback::session("sticky-real").await;
        let peer = tokio::spawn(async move {
            let mut peer = accept_async(listener.accept().await.unwrap().0)
                .await
                .unwrap();
            super::loopback::read_request(&mut peer).await;
            let _ = peer.close(None).await;
            while peer.next().await.is_some() {}
            let mut peer = accept_async(listener.accept().await.unwrap().0)
                .await
                .unwrap();
            super::loopback::answer(&mut peer, "restored", "back").await;
            while peer.next().await.is_some() {}
        });
        let stats = || get_openai_codex_web_socket_debug_stats("sticky-real").unwrap();
        assert_eq!(session.run().await, StopReason::Stop);
        let first = stats();
        assert_eq!((first.websocket_failures, first.sse_fallbacks), (1, 1));
        assert_eq!(session.run().await, StopReason::Stop);
        let second = stats();
        assert_eq!((second.websocket_failures, second.sse_fallbacks), (1, 2));
        assert_eq!(session.http_calls(), 2);
        reset_openai_codex_web_socket_debug_stats(Some("sticky-real"));
        assert_eq!(session.run().await, StopReason::Stop);
        assert_eq!(session.http_calls(), 2);
        close_openai_codex_web_socket_sessions(Some("sticky-real"));
        peer.await.unwrap();
    });
}

#[test]
fn maestro_response_sessions_scope_fallback_by_socket_history() {
    let _guard = super::exclusive();
    crate::tests::runtime::run(
        maestro_response_sessions_scope_fallback_by_socket_history_scenario(),
    );
}

/// The permanent fallback covers sessions whose socket never completed a response; a session that
/// completed one falls back for a single request and returns to the socket.
async fn maestro_response_sessions_scope_fallback_by_socket_history_scenario() {
    reset_openai_codex_web_socket_debug_stats(None);
    transient_failure().await;
    unserved_socket().await;
    reset_openai_codex_web_socket_debug_stats(None);
}

/// One completed response, one socket drop before the first event, then a socket again.
async fn transient_failure() {
    let (listener, session) = Fallback::session("transient").await;
    let peer = tokio::spawn(async move {
        let mut peer = accept_async(listener.accept().await.unwrap().0)
            .await
            .unwrap();
        super::loopback::answer(&mut peer, "first", "one").await;
        super::loopback::read_request(&mut peer).await;
        let _ = peer.close(None).await;
        while peer.next().await.is_some() {}
        let mut peer = accept_async(listener.accept().await.unwrap().0)
            .await
            .unwrap();
        super::loopback::answer(&mut peer, "third", "three").await;
        while peer.next().await.is_some() {}
    });
    let stats = || get_openai_codex_web_socket_debug_stats("transient").unwrap();
    assert_eq!(session.run().await, StopReason::Stop);
    assert_eq!(session.http_calls(), 0);
    assert_eq!(session.run().await, StopReason::Stop);
    let fallen_back = stats();
    assert_eq!(
        (
            fallen_back.websocket_failures,
            fallen_back.sse_fallbacks,
            fallen_back.websocket_fallback_active
        ),
        (1, 1, Some(false))
    );
    assert_eq!(session.http_calls(), 1);
    assert_eq!(session.run().await, StopReason::Stop);
    assert_eq!(
        session.http_calls(),
        1,
        "the next request tries the socket again"
    );
    close_openai_codex_web_socket_sessions(Some("transient"));
    peer.await.unwrap();
}

/// A socket that never completes an upgrade keeps the HTTP stream for every request.
async fn unserved_socket() {
    let (listener, session) = Fallback::session("unserved").await;
    let peer = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        drop(stream);
        drop(listener);
    });
    for _ in 0..2 {
        assert_eq!(session.run().await, StopReason::Stop);
    }
    let stats = get_openai_codex_web_socket_debug_stats("unserved").unwrap();
    assert_eq!(
        (
            stats.websocket_failures,
            stats.sse_fallbacks,
            stats.websocket_fallback_active
        ),
        (1, 2, Some(true))
    );
    assert_eq!(session.http_calls(), 2);
    close_openai_codex_web_socket_sessions(Some("unserved"));
    peer.await.unwrap();
}

#[test]
fn maestro_response_sessions_cleanup_registered_sockets() {
    let _guard = super::exclusive();
    crate::tests::runtime::run(maestro_response_sessions_cleanup_registered_sockets_scenario());
}

/// Execute the controlled public-operation scenario.
async fn maestro_response_sessions_cleanup_registered_sockets_scenario() {
    for all in [None, Some("")] {
        reset_openai_codex_web_socket_debug_stats(None);
        let mut first = retained("cleanup-first").await;
        let mut second = retained("cleanup-second").await;
        let stats = get_openai_codex_web_socket_debug_stats("cleanup-first").unwrap();
        crate::cleanup_session_resources(Some("cleanup-first")).unwrap();
        closed(&mut first.peer).await;
        assert_eq!(
            get_openai_codex_web_socket_debug_stats("cleanup-first"),
            Some(stats)
        );
        let stream =
            stream_openai_codex_responses(second.model, second.context, Some(second.options));
        let (_, output) = join(
            super::loopback::answer(&mut second.peer, "kept", "not selected"),
            stream.result(),
        )
        .await;
        assert_eq!(output.read().unwrap().stop_reason, StopReason::Stop);
        assert_eq!(
            get_openai_codex_web_socket_debug_stats("cleanup-second")
                .unwrap()
                .connections_reused,
            1
        );
        crate::cleanup_session_resources(all).unwrap();
        closed(&mut second.peer).await;
        crate::cleanup_session_resources(all).unwrap();
    }
}

#[test]
fn maestro_response_sessions_empty_sessions_stay_uncached() {
    let _guard = super::exclusive();
    crate::tests::runtime::run(maestro_response_sessions_empty_sessions_stay_uncached_scenario());
}

/// Execute the controlled public-operation scenario.
async fn maestro_response_sessions_empty_sessions_stay_uncached_scenario() {
    for session in [None, Some(""), Some(" ")] {
        empty_session(session).await;
    }
}

/// Observe one public call while the session has retained fallback state.
async fn sticky_mode(mode: Option<Transport>) {
    let before = get_openai_codex_web_socket_debug_stats("sticky-public").unwrap();
    let (model, context, mut options) = super::invocation();
    options.common.session_id = Some("sticky-public".into());
    options.common.transport = mode.clone();
    options.common.fetch = Some(Arc::new(|_| {
        Box::pin(async { Ok(super::public_streams::response()) })
    }));
    let output = stream_openai_codex_responses((*model).clone(), context, Some(options))
        .result()
        .await;
    assert_eq!(output.read().unwrap().stop_reason, StopReason::Stop);
    assert!(output.read().unwrap().diagnostics.is_none());
    let after = get_openai_codex_web_socket_debug_stats("sticky-public").unwrap();
    assert_eq!(
        after.sse_fallbacks - before.sse_fallbacks,
        u64::from(!matches!(mode, Some(Transport::Sse)))
    );
    assert_eq!(after.requests, before.requests);
    assert_eq!(after.websocket_failures, 1);
}

/// Observe two calls with one absent, empty or whitespace session.
async fn empty_session(session: Option<&'static str>) {
    reset_openai_codex_web_socket_debug_stats(None);
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (model, context, mut options) = super::invocation();
    let mut model = (*model).clone();
    model.base_url = format!("http://{}", listener.local_addr().unwrap());
    options.common.session_id = session.map(str::to_owned);
    let peer = tokio::spawn(async move {
        let mut peer = accept_async(listener.accept().await.unwrap().0)
            .await
            .unwrap();
        super::loopback::answer(&mut peer, "first", "one").await;
        if session != Some(" ") {
            while peer.next().await.is_some() {}
            peer = accept_async(listener.accept().await.unwrap().0)
                .await
                .unwrap();
        }
        super::loopback::answer(&mut peer, "second", "two").await;
        while peer.next().await.is_some() {}
    });
    for _ in 0..2 {
        let output =
            stream_openai_codex_responses(model.clone(), context.clone(), Some(options.clone()))
                .result()
                .await;
        assert_eq!(output.read().unwrap().stop_reason, StopReason::Stop);
    }
    close_openai_codex_web_socket_sessions(session);
    peer.await.unwrap();
    if session == Some(" ") {
        let stats = get_openai_codex_web_socket_debug_stats(" ").unwrap();
        assert_eq!(
            (
                stats.requests,
                stats.connections_created,
                stats.connections_reused
            ),
            (2, 1, 1)
        );
    } else {
        assert!(get_openai_codex_web_socket_debug_stats("").is_none());
        super::super::debug::record_web_socket_failure(
            session,
            crate::DiagnosticInput::Text("controlled failure"),
        );
        assert!(!super::super::debug::is_web_socket_sse_fallback_active(
            session
        ));
        assert!(get_openai_codex_web_socket_debug_stats("").is_none());
    }
}
