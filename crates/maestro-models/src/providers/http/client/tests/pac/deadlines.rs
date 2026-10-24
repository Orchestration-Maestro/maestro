//! PAC candidate establishment shares one connect budget; headers do not bound bodies.

use super::{request, routed};
use crate::providers::http::proxy_agent::tests::{
    deadlines::{pending, ready},
    socks,
};
use crate::providers::http::{FetchError, Route, collect_body, configured_fetch};
use crate::tests::loopback;
use futures_util::FutureExt;
use std::sync::{Arc, mpsc};
use std::time::Duration;

/// Supply independent budgets without changing the retained PAC source owner.
fn timed(script: &str, connect: u64, headers: u64) -> crate::Fetch {
    let encoded = percent_encoding::utf8_percent_encode(script, percent_encoding::NON_ALPHANUMERIC);
    let source = url::Url::parse(&format!("pac+data:,{encoded}")).unwrap();
    configured_fetch(Route {
        proxy: Some(Arc::new(move |_| Ok(Some(source.clone())))),
        connect_timeout: Some(Duration::from_secs(connect)),
        read_timeout: Some(Duration::from_secs(headers)),
        ..Route::default()
    })
}

#[test]
fn pac_connect_and_header_deadlines_keep_distinct_scopes() {
    crate::tests::runtime::run(async {
        tokio::time::pause();
        for (connect, headers) in [(3, 10), (10, 3)] {
            boundary(connect, headers, false).await;
            boundary(connect, headers, true).await;
        }
        candidate_budget().await;
        handshake_budget().await;
    });
}

/// Stall either negotiation or headers and inspect both sides of its supplied deadline.
async fn boundary(connect: u64, headers: u64, negotiated: bool) {
    let (sent, received) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let released = std::sync::Mutex::new(released);
    let server = loopback::start_routed(Arc::new(move |mut socket| {
        use std::io::Read;
        if negotiated {
            socks::negotiate(&mut socket, false)?;
            assert_eq!(
                loopback::read_request(&mut socket)?.unwrap().body,
                b"unique body"
            );
        } else {
            let mut greeting = [0; 3];
            socket.read_exact(&mut greeting)?;
            assert_eq!(greeting, [5, 1, 0]);
        }
        sent.send(()).unwrap();
        released.lock().unwrap().recv().unwrap();
        assert!(socks::failed_frames(&mut socket, &[])?.is_empty());
        Ok(())
    }))
    .unwrap();
    let fetch = timed(
        &format!(
            "function FindProxyForURL(){{return 'SOCKS {}'}}",
            &server.url[7..]
        ),
        connect,
        headers,
    );
    let mut future = fetch(request("http://destination.invalid/".into()));
    ready(&received, &mut future).await;
    let deadline = if negotiated {
        headers
    } else {
        connect.min(headers)
    };
    tokio::time::advance(Duration::from_secs(deadline - 1)).await;
    assert!(pending(&mut future).await);
    tokio::time::advance(Duration::from_secs(2)).await;
    assert!(matches!(
        future.as_mut().now_or_never(),
        Some(Err(FetchError::Timeout))
    ));
    drop(future);
    release.send(()).unwrap();
    drop(fetch);
    server.finish().unwrap();
}

/// A second candidate uses the remainder of the existing connect budget, not a fresh one.
async fn candidate_budget() {
    let (sent, first) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let released = std::sync::Mutex::new(released);
    let server = loopback::start_routed(Arc::new(move |mut socket| {
        use std::io::Read;
        let mut greeting = [0; 3];
        socket.read_exact(&mut greeting)?;
        sent.send(()).unwrap();
        released.lock().unwrap().recv().unwrap();
        Ok(())
    }))
    .unwrap();
    let (sent, second) = mpsc::channel();
    let (finish, finished) = mpsc::channel();
    let finished = std::sync::Mutex::new(finished);
    let next = loopback::start_routed(Arc::new(move |mut socket| {
        use std::io::Read;
        let mut greeting = [0; 3];
        socket.read_exact(&mut greeting)?;
        sent.send(()).unwrap();
        finished.lock().unwrap().recv().unwrap();
        assert!(socks::failed_frames(&mut socket, &[])?.is_empty());
        Ok(())
    }))
    .unwrap();
    let fetch = timed(
        &format!(
            "function FindProxyForURL(){{return 'SOCKS {}; SOCKS {}'}}",
            &server.url[7..],
            &next.url[7..]
        ),
        10,
        40,
    );
    let mut future = fetch(request("http://destination.invalid/".into()));
    ready(&first, &mut future).await;
    tokio::time::advance(Duration::from_secs(6)).await;
    assert!(pending(&mut future).await);
    release.send(()).unwrap();
    ready(&second, &mut future).await;
    tokio::time::advance(Duration::from_secs(3)).await;
    assert!(pending(&mut future).await);
    tokio::time::advance(Duration::from_secs(2)).await;
    assert!(matches!(
        future.as_mut().now_or_never(),
        Some(Err(FetchError::Timeout))
    ));
    drop(future);
    finish.send(()).unwrap();
    server.finish().unwrap();
    next.finish().unwrap();
}

/// With no supplied budgets, the SOCKS owner's independent handshake timer still applies.
async fn handshake_budget() {
    let (sent, received) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let released = std::sync::Mutex::new(released);
    let server = loopback::start_routed(Arc::new(move |mut socket| {
        use std::io::Read;
        let mut greeting = [0; 3];
        socket.read_exact(&mut greeting)?;
        sent.send(()).unwrap();
        released.lock().unwrap().recv().unwrap();
        assert!(socks::failed_frames(&mut socket, &[])?.is_empty());
        Ok(())
    }))
    .unwrap();
    let fetch = routed(&format!(
        "function FindProxyForURL(){{return 'SOCKS {}'}}",
        &server.url[7..]
    ));
    let mut future = fetch(request("http://destination.invalid/".into()));
    ready(&received, &mut future).await;
    tokio::time::advance(Duration::from_secs(29)).await;
    assert!(pending(&mut future).await);
    tokio::time::advance(Duration::from_secs(2)).await;
    assert!(
        future
            .as_mut()
            .now_or_never()
            .unwrap()
            .err()
            .unwrap()
            .to_string()
            .contains("Failed to establish")
    );
    drop(future);
    release.send(()).unwrap();
    server.finish().unwrap();
}

#[test]
fn pac_response_body_outlives_header_deadline() {
    crate::tests::runtime::run(async {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        tokio::time::pause();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (release, released) = tokio::sync::oneshot::channel();
        let peer = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            while !bytes.ends_with(b"unique body") {
                bytes.push(socket.read_u8().await.unwrap());
            }
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\n")
                .await
                .unwrap();
            released.await.unwrap();
            socket.write_all(b"late").await.unwrap();
        });
        let fetch = timed(
            &format!("function FindProxyForURL(){{return 'PROXY {address}'}}"),
            3,
            10,
        );
        let mut future = fetch(request("http://destination.invalid/".into()));
        let response = loop {
            if let Some(response) = future.as_mut().now_or_never() {
                break response.unwrap();
            }
            tokio::task::yield_now().await;
        };
        assert_eq!(response.status, 200);
        let mut body = Box::pin(collect_body(response.body));
        tokio::time::advance(Duration::from_secs(11)).await;
        assert!(pending(&mut body).await);
        release.send(()).unwrap();
        assert_eq!(body.await.unwrap(), b"late");
        peer.await.unwrap();
    });
}
