//! Shared-client idle limits against a held local HTTP peer.

use crate::{BoxFuture, FetchError, HttpRequest, HttpResponse};
use futures_util::{
    FutureExt,
    future::{Either, select},
};
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::sync::oneshot;

/// A request whose local peer has read the headers but withholds its response.
async fn held_headers(
    fetch: crate::Fetch,
) -> (
    BoxFuture<Result<HttpResponse, FetchError>>,
    oneshot::Sender<()>,
    tokio::task::JoinHandle<()>,
) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (arrived, received) = oneshot::channel();
    let (release, released) = oneshot::channel();
    let peer = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut headers = Vec::new();
        while !headers.ends_with(b"\r\n\r\n") {
            headers.push(socket.read_u8().await.unwrap());
        }
        arrived.send(()).unwrap();
        released.await.unwrap();
    });
    let request = fetch(HttpRequest {
        method: "GET".into(),
        url: format!("http://{address}/"),
        headers: indexmap::IndexMap::new(),
        body: Vec::new(),
        signal: None,
    });
    match select(request, received).await {
        Either::Right((Ok(()), request)) => (request, release, peer),
        _ => panic!("the peer must receive the request before it ends"),
    }
}

#[test]
fn shared_http_client_starts_with_a_five_minute_header_limit() {
    crate::tests::runtime::run(header_limit(crate::default_fetch(), Some(300_000)));
}

/// Observe one header wait after the peer has received the request.
async fn header_limit(fetch: crate::Fetch, milliseconds: Option<u64>) {
    let (mut request, release, peer) = held_headers(fetch).await;
    tokio::time::pause();
    let started = tokio::time::Instant::now();
    let limit = milliseconds.unwrap_or(600_000);
    let early = limit * 3 / 4;
    tokio::time::advance(Duration::from_millis(early)).await;
    tokio::task::yield_now().await;
    assert!(request.as_mut().now_or_never().is_none());
    tokio::time::advance(Duration::from_millis(limit - early)).await;
    tokio::task::yield_now().await;
    if milliseconds.is_some() {
        assert!(matches!(request.as_mut().await, Err(FetchError::Timeout)));
        // Tokio deadlines round up to the next 1 ms timer tick.
        assert!(
            (u128::from(limit)..=u128::from(limit) + 1).contains(&started.elapsed().as_millis())
        );
    } else {
        assert!(request.as_mut().now_or_never().is_none());
    }
    drop(request);
    release.send(()).unwrap();
    peer.await.unwrap();
    tokio::time::resume();
}

#[test]
fn shared_http_idle_limit_changes_the_next_request_from_an_existing_fetch() {
    crate::tests::runtime::run(async {
        let fetch = crate::default_fetch();
        for milliseconds in [30_000_u32, 60_000] {
            crate::configure_http_dispatcher(f64::from(milliseconds)).unwrap();
            header_limit(fetch.clone(), Some(u64::from(milliseconds))).await;
        }
    });
}

#[test]
fn zero_disables_shared_http_header_limits_and_failed_configuration_keeps_it() {
    crate::tests::runtime::run(async {
        for milliseconds in [0.0, 0.9] {
            crate::configure_http_dispatcher(milliseconds).unwrap();
            assert!(crate::configure_http_dispatcher(f64::MAX).is_err());
            header_limit(crate::default_fetch(), None).await;
        }
    });
}

/// A response that has delivered one byte and withholds the next body chunk.
async fn held_body() -> (
    crate::HttpBody,
    oneshot::Sender<()>,
    tokio::task::JoinHandle<()>,
) {
    use futures_util::StreamExt;
    use tokio::io::AsyncWriteExt;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (release, released) = oneshot::channel();
    let peer = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut headers = Vec::new();
        while !headers.ends_with(b"\r\n\r\n") {
            headers.push(socket.read_u8().await.unwrap());
        }
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\na")
            .await
            .unwrap();
        released.await.unwrap();
    });
    let mut response = crate::default_fetch()(HttpRequest {
        method: "GET".into(),
        url: format!("http://{address}/"),
        headers: indexmap::IndexMap::new(),
        body: Vec::new(),
        signal: None,
    })
    .await
    .unwrap();
    assert_eq!(response.body.next().await.unwrap().unwrap(), b"a");
    (response.body, release, peer)
}

#[test]
fn shared_http_idle_limit_bounds_body_chunks_and_zero_disables_it() {
    use futures_util::StreamExt;
    crate::tests::runtime::run(async {
        for milliseconds in [60_000, 0] {
            crate::configure_http_dispatcher(f64::from(milliseconds)).unwrap();
            let (mut body, release, peer) = held_body().await;
            tokio::time::pause();
            let started = tokio::time::Instant::now();
            let mut next = body.next();
            assert!((&mut next).now_or_never().is_none());
            tokio::time::advance(Duration::from_secs(45)).await;
            assert!((&mut next).now_or_never().is_none());
            tokio::time::advance(Duration::from_secs(15)).await;
            tokio::task::yield_now().await;
            if milliseconds == 0 {
                tokio::time::advance(Duration::from_secs(600)).await;
                assert!((&mut next).now_or_never().is_none());
            } else {
                assert_eq!((&mut next).await, Some(Err(FetchError::Timeout)));
                // Tokio deadlines round up to the next 1 ms timer tick.
                assert!((60_000..=60_001).contains(&started.elapsed().as_millis()));
            }
            drop(next);
            drop(body);
            release.send(()).unwrap();
            peer.await.unwrap();
            tokio::time::resume();
        }
    });
}
