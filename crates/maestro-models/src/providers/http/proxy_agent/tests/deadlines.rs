//! Controlled clock witnesses at connection and header boundaries.

use super::run;
use crate::tests::{loopback, socks};
use futures_util::FutureExt;
use std::sync::{Arc, mpsc};
use std::time::Duration;
use tower_service::Service;

/// Keep paused time runnable until a peer reaches a named protocol boundary.
async fn ready(
    received: &mpsc::Receiver<()>,
    future: &mut std::pin::Pin<Box<impl std::future::Future + ?Sized>>,
) {
    while received.try_recv().is_err() {
        assert!(pending(future).await);
        tokio::task::yield_now().await;
    }
}

/// Poll the same operation once without advancing time or installing a test deadline.
async fn pending(future: &mut std::pin::Pin<Box<impl std::future::Future + ?Sized>>) -> bool {
    std::future::poll_fn(|context| {
        std::task::Poll::Ready(future.as_mut().poll(context).is_pending())
    })
    .await
}

/// Observe either the first greeting or TLS hello after a granted tunnel.
fn observe_stall(socket: &mut std::net::TcpStream, tunnel: bool) -> std::io::Result<()> {
    use std::io::Read;
    if tunnel {
        socks::negotiate(socket, false)?;
        let mut hello = [0];
        socket.read_exact(&mut hello)?;
        assert_eq!(hello, [22]);
    } else {
        let mut hello = [0; 3];
        socket.read_exact(&mut hello)?;
        assert_eq!(hello, [5, 1, 0]);
    }
    Ok(())
}

#[test]
fn socks_handshake_deadline_ends_at_tunnel_establishment() {
    run(async {
        tokio::time::pause();
        for tunnel in [false, true] {
            let (sent, received) = mpsc::channel();
            let (release, released) = mpsc::channel();
            let (ended, end) = mpsc::channel();
            let released = std::sync::Mutex::new(released);
            let server = loopback::start_routed(Arc::new(move |mut socket| {
                use std::io::Read;
                observe_stall(&mut socket, tunnel)?;
                sent.send(()).unwrap();
                released.lock().unwrap().recv().unwrap();
                let mut rest = Vec::new();
                socket.read_to_end(&mut rest)?;
                ended.send(rest).unwrap();
                Ok(())
            }))
            .unwrap();
            let proxy = url::Url::parse(&server.url.replacen("http", "socks5h", 1)).unwrap();
            let uri = if tunnel {
                "https://destination.invalid/"
            } else {
                "http://destination.invalid/"
            }
            .parse()
            .unwrap();
            let mut connector = hyper_rustls::HttpsConnector::from((
                super::super::ProxyAgent {
                    select: Arc::new(move |_| Ok(Some(proxy.clone()))),
                },
                super::super::tls(true).unwrap(),
            ));
            let mut future = connector.call(uri).boxed();
            ready(&received, &mut future).await;
            tokio::time::advance(Duration::from_secs(31)).await;
            if tunnel {
                assert!(pending(&mut future).await);
                drop(future);
            } else {
                let error = future.await.expect_err("handshake expires");
                assert!(error.to_string().contains("SOCKS handshake deadline"));
            }
            release.send(()).unwrap();
            server.finish().unwrap();
            end.recv().unwrap();
        }
    });
}

#[test]
fn socks_connect_and_header_deadlines_keep_separate_scopes() {
    run(async {
        tokio::time::pause();
        for (connect, headers, phase) in [
            (3, 10, Phase::Handshake),
            (10, 3, Phase::Handshake),
            (3, 10, Phase::Headers),
            (3, 10, Phase::Tls),
            (10, 3, Phase::Tls),
        ] {
            deadline_exchange(connect, headers, phase).await;
        }
    });
}

/// Peer boundary at which the request is held pending.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// SOCKS greeting has arrived without a reply.
    Handshake,
    /// The tunnel is established and the HTTP request has arrived.
    Headers,
    /// The tunnel is established and destination TLS has started.
    Tls,
}

/// Compare independently supplied deadlines at handshake, TLS and header phases.
async fn deadline_exchange(connect: u64, headers: u64, phase: Phase) {
    let (sent, received) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let released = std::sync::Mutex::new(released);
    let server = loopback::start_routed(Arc::new(move |mut socket| {
        use std::io::Read;
        if phase == Phase::Headers {
            socks::negotiate(&mut socket, false)?;
            assert!(loopback::read_request(&mut socket)?.is_some());
        } else {
            observe_stall(&mut socket, phase == Phase::Tls)?;
        }
        sent.send(()).unwrap();
        released.lock().unwrap().recv().unwrap();
        let mut bytes = Vec::new();
        socket.read_to_end(&mut bytes)?;
        if phase != Phase::Tls {
            assert!(bytes.is_empty());
        }
        Ok(())
    }))
    .unwrap();
    let proxy = url::Url::parse(&server.url.replacen("http", "socks5h", 1)).unwrap();
    let fetch = crate::providers::http::configured_fetch(crate::providers::http::Route {
        proxy: Some(Arc::new(move |_| Ok(Some(proxy.clone())))),
        http1_only: true,
        connect_timeout: Some(Duration::from_secs(connect)),
        read_timeout: Some(Duration::from_secs(headers)),
    });
    let destination = if phase == Phase::Tls {
        "https://destination.invalid/"
    } else {
        "http://destination.invalid/"
    };
    let mut future = fetch(super::request(destination));
    ready(&received, &mut future).await;
    tokio::time::advance(Duration::from_secs(4)).await;
    if phase == Phase::Headers {
        assert!(pending(&mut future).await);
        tokio::time::advance(Duration::from_secs(7)).await;
    }
    let outcome = future.as_mut().now_or_never();
    drop(future);
    release.send(()).unwrap();
    drop(fetch);
    server.finish().unwrap();
    assert!(
        matches!(
            outcome,
            Some(Err(crate::providers::http::FetchError::Timeout))
        ),
        "shorter deadline must have expired at the observed clock boundary"
    );
}
