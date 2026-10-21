//! Controlled candidate traces and application-byte ownership.

use super::{request, routed};
use crate::providers::http::collect_body;
use crate::tests::{loopback, socks};
use std::sync::Arc;

#[test]
fn pac_http_proxies_preserve_absolute_form_and_auth() {
    crate::tests::runtime::run(async {
        for directive in ["PROXY", "HTTP"] {
            let (sent, received) = std::sync::mpsc::channel();
            let server = loopback::serve_routed(move |request| {
                sent.send((
                    request.request_line.clone(),
                    request.body.clone(),
                    request.header("proxy-authorization").map(str::to_owned),
                ))
                .unwrap();
                loopback::Reply {
                    status: 403,
                    content_type: "text/plain",
                    headers: &[],
                    body: b"final refusal".to_vec(),
                    close: false,
                }
            })
            .unwrap();
            let script = format!(
                "function FindProxyForURL(){{return '{directive} u%3A%40:p%3A%40@{}; DIRECT'}}",
                &server.url[7..]
            );
            let response = routed(&script)(request(
                "http://destination.invalid:8081/once?q=1&q=2".into(),
            ))
            .await
            .unwrap();
            assert_eq!(response.status, 403);
            assert_eq!(collect_body(response.body).await.unwrap(), b"final refusal");
            server.finish().unwrap();
            let seen: Vec<_> = received.try_iter().collect();
            assert_eq!(
                seen,
                [(
                    "POST http://destination.invalid:8081/once?q=1&q=2 HTTP/1.1".into(),
                    b"unique body".to_vec(),
                    Some("Basic dTpAOnA6QA==".into())
                )]
            );
        }
    });
}

#[test]
fn pac_socks_routes_delegate_negotiation_and_remote_dns() {
    crate::tests::runtime::run(async {
        for directive in ["SOCKS", "SOCKS5", "SOCKS4"] {
            socks_exchange(directive).await;
        }
    });
}

/// Inspect a granted proxy negotiation before the single application request.
async fn socks_exchange(directive: &str) {
    let version4 = directive == "SOCKS4";
    let (sent, received) = std::sync::mpsc::channel();
    let server = loopback::start_routed(Arc::new(move |mut socket| {
        let address = socks::negotiate(&mut socket, !version4)?;
        assert_eq!(address.kind, 3);
        assert_eq!(address.host, b"destination.invalid");
        assert_eq!(address.port, 8081);
        assert_eq!(address.user.as_deref(), Some(b"u:@".as_slice()));
        assert_eq!(
            address.password.as_deref(),
            if version4 {
                None
            } else {
                Some(b"p:@".as_slice())
            }
        );
        loopback::answer_connection(socket, &|request| {
            assert!(request.header("proxy-authorization").is_none());
            sent.send((request.request_line.clone(), request.body.clone()))
                .unwrap();
            loopback::Reply {
                status: 200,
                content_type: "text/plain",
                headers: &[],
                body: b"done".to_vec(),
                close: false,
            }
        })
    }))
    .unwrap();
    let script = format!(
        "function FindProxyForURL(){{return '{directive} u%3A%40:p%3A%40@{}'}}",
        &server.url[7..]
    );
    let response = routed(&script)(request("http://destination.invalid:8081/once".into()))
        .await
        .unwrap();
    assert_eq!(collect_body(response.body).await.unwrap(), b"done");
    server.finish().unwrap();
    assert_eq!(
        received.try_iter().collect::<Vec<_>>(),
        [("POST /once HTTP/1.1".into(), b"unique body".to_vec())]
    );
}

/// Refuse each complete SOCKS request and retain its ordered destination before closing.
fn refusing_peer(
    label: u8,
    trace: std::sync::mpsc::Sender<u8>,
) -> (loopback::Routed, std::sync::mpsc::Receiver<Vec<u8>>) {
    let (sent, received) = std::sync::mpsc::channel();
    let server = loopback::start_routed(Arc::new(move |mut socket| {
        let address = socks::negotiate_reply(&mut socket, false, &[5, 5, 0, 1, 0, 0, 0, 0, 0, 0])?;
        trace.send(label).unwrap();
        sent.send(address.host).unwrap();
        assert!(socks::failed_frames(&mut socket, &[])?.is_empty());
        Ok(())
    }))
    .unwrap();
    (server, received)
}

#[test]
fn pac_candidates_keep_order_aliases_and_duplicates() {
    crate::tests::runtime::run(async {
        let (trace, ordered) = std::sync::mpsc::channel();
        let (first, first_seen) = refusing_peer(1, trace.clone());
        let (second, second_seen) = refusing_peer(2, trace);
        let script = format!(
            "function FindProxyForURL(){{return 'SOCKS {};SOCKS5 {};SOCKS {};SOCKS5 {}'}}",
            &first.url[7..],
            &second.url[7..],
            &first.url[7..],
            &second.url[7..]
        );
        let error = routed(&script)(request("http://destination.invalid:8081/".into()))
            .await
            .err()
            .unwrap();
        assert!(error.to_string().contains("Failed to establish"));
        first.finish().unwrap();
        second.finish().unwrap();
        assert_eq!(ordered.try_iter().collect::<Vec<_>>(), [1, 2, 1, 2]);
        assert_eq!(
            first_seen.try_iter().collect::<Vec<_>>(),
            vec![b"destination.invalid".to_vec(); 2]
        );
        assert_eq!(
            second_seen.try_iter().collect::<Vec<_>>(),
            vec![b"destination.invalid".to_vec(); 2]
        );
    });
}

#[test]
fn pac_candidate_walk_has_linear_work() {
    crate::tests::runtime::run(async {
        for count in [16, 32, 64] {
            let (trace, _ordered) = std::sync::mpsc::channel();
            let (server, observed) = refusing_peer(1, trace);
            let candidate = format!("SOCKS {}", &server.url[7..]);
            let candidates = vec![candidate; count].join(";");
            let script = format!("function FindProxyForURL(){{return '{candidates}'}}");
            let error = routed(&script)(request("http://destination.invalid/".into()))
                .await
                .err()
                .unwrap();
            assert!(error.to_string().contains("Failed to establish"));
            server.finish().unwrap();
            let attempts = observed.try_iter().count();
            assert!(attempts > 0);
            assert!(
                attempts <= count,
                "{attempts} attempts for {count} candidates"
            );
        }
    });
}

#[test]
fn pac_selected_failures_never_replay_transmitted_bodies() {
    crate::tests::runtime::run(async {
        let (sent, received) = std::sync::mpsc::channel();
        let selected = loopback::start_routed(Arc::new(move |mut socket| {
            let request = loopback::read_request(&mut socket)?.unwrap();
            sent.send(request.body).unwrap();
            Ok(())
        }))
        .unwrap();
        let (sent, fallback_seen) = std::sync::mpsc::channel();
        let fallback = loopback::serve_routed(move |_| {
            sent.send(()).unwrap();
            loopback::Reply {
                status: 200,
                content_type: "text/plain",
                headers: &[],
                body: b"replayed".to_vec(),
                close: false,
            }
        })
        .unwrap();
        let script = format!(
            "function FindProxyForURL(){{return 'PROXY {};DIRECT'}}",
            &selected.url[7..]
        );
        let result = routed(&script)(request(fallback.url.clone())).await;
        selected.finish().unwrap();
        fallback.finish().unwrap();
        assert!(result.is_err());
        assert_eq!(
            received.try_iter().collect::<Vec<_>>(),
            [b"unique body".to_vec()]
        );
        assert_eq!(fallback_seen.try_iter().count(), 0);
    });
}

#[test]
fn pac_cancel_and_drop_release_selected_connections() {
    crate::tests::runtime::run(async {
        for after_request in [false, true] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let proxy = listener.local_addr().unwrap();
            let (sent, ready) = tokio::sync::oneshot::channel();
            let peer = tokio::spawn(async move {
                let (mut socket, _) = listener.accept().await.unwrap();
                cancellation_peer(&mut socket, after_request, sent).await;
            });
            let directive = if after_request { "HTTP" } else { "SOCKS" };
            let script = format!("function FindProxyForURL(){{return '{directive} {proxy}'}}");
            let fetch = routed(&script);
            let sending = tokio::spawn(fetch(request("http://destination.invalid/".into())));
            ready.await.unwrap();
            sending.abort();
            assert!(sending.await.err().unwrap().is_cancelled());
            peer.await.unwrap();
            drop(fetch);
        }
    });
}

/// Signal the reached phase before observing closure of the selected socket.
async fn cancellation_peer(
    socket: &mut tokio::net::TcpStream,
    after_request: bool,
    sent: tokio::sync::oneshot::Sender<()>,
) {
    use tokio::io::AsyncReadExt;
    if after_request {
        let mut bytes = Vec::new();
        while !bytes.ends_with(b"unique body") {
            bytes.push(socket.read_u8().await.unwrap());
        }
    } else {
        let mut greeting = [0; 3];
        socket.read_exact(&mut greeting).await.unwrap();
        assert_eq!(greeting, [5, 1, 0]);
    }
    sent.send(()).unwrap();
    assert_eq!(socket.read(&mut [0]).await.unwrap(), 0);
}
