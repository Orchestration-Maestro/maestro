//! Controlled native SOCKS transport behavior.

use crate::tests::loopback;

pub(in crate::providers::http) mod socks;

use crate::providers::http::{HttpRequest, Route, configured_fetch};
use crate::tests::runtime::run;
use futures_util::StreamExt;
use std::sync::Arc;

/// Select the supplied proxy without consulting process environment.
fn sender(proxy: url::Url) -> crate::providers::http::Fetch {
    configured_fetch(Route {
        proxy: Some(Arc::new(move |_| Ok(Some(proxy.clone())))),
        http1_only: true,
        ..Route::default()
    })
}

/// A request with distinguishable fields.
fn request(url: &str) -> HttpRequest {
    HttpRequest {
        method: "POST".into(),
        url: url.into(),
        headers: indexmap::IndexMap::from([("x-input".into(), "retained".into())]),
        body: b"payload".to_vec(),
        signal: None,
    }
}

/// Capture a completed SOCKS and HTTP exchange.
async fn exchange(
    scheme: &str,
    destination: &str,
    credentials: &str,
    authenticate: bool,
) -> socks::Negotiation {
    exchange_bound(
        scheme,
        destination,
        credentials,
        authenticate,
        &[5, 0, 0, 1, 127, 0, 0, 1, 0, 80],
    )
    .await
}

/// Capture a tunnel with an independently selected bound-address frame.
async fn exchange_bound(
    scheme: &str,
    destination: &str,
    credentials: &str,
    authenticate: bool,
    reply: &[u8],
) -> socks::Negotiation {
    let reply = reply.to_vec();
    let (sent, received) = std::sync::mpsc::channel();
    let server = loopback::start_routed(Arc::new(move |mut socket| {
        let negotiation = socks::negotiate_reply(&mut socket, authenticate, &reply)?;
        sent.send(negotiation).unwrap();
        loopback::answer_connection(socket, &|_| loopback::Reply {
            status: 200,
            content_type: "text/plain",
            headers: &[],
            body: b"reply".to_vec(),
            close: false,
        })
    }))
    .unwrap();
    let proxy = url::Url::parse(&format!(
        "{scheme}://{credentials}{}",
        server.url.strip_prefix("http://").unwrap()
    ))
    .unwrap();
    let mut response = sender(proxy)(request(destination)).await.unwrap();
    let mut bytes = Vec::new();
    while let Some(chunk) = response.body.next().await {
        bytes.extend(chunk.unwrap());
    }
    assert_eq!(bytes, b"reply");
    server.finish().unwrap();
    received.recv().unwrap()
}

#[test]
fn socks_schemes_choose_local_or_remote_dns() {
    run(async {
        for scheme in ["socks", "socks5h"] {
            let observed =
                exchange(scheme, "http://destination.invalid:8081/path", "", false).await;
            assert_eq!(observed.kind, 3);
            assert_eq!(observed.host, b"destination.invalid");
            assert_eq!(observed.port, 8081);
        }
        let observed = exchange("socks5", "http://localhost/path", "", false).await;
        assert!(matches!(observed.kind, 1 | 4));
        let addresses: Vec<_> = tokio::net::lookup_host(("localhost", 80))
            .await
            .unwrap()
            .map(|address| address.ip())
            .collect();
        let address = match observed.kind {
            1 => std::net::IpAddr::from(<[u8; 4]>::try_from(observed.host).unwrap()),
            _ => std::net::IpAddr::from(<[u8; 16]>::try_from(observed.host).unwrap()),
        };
        assert!(addresses.contains(&address));
        assert_eq!(observed.port, 80);
    });
}

#[test]
fn socks_credentials_offer_no_login_and_preserve_empty_fields() {
    run(async {
        for credentials in ["", ":@"] {
            let observed =
                exchange("socks", "http://destination.invalid/", credentials, false).await;
            assert_eq!(observed.methods, [0]);
            assert!(observed.user.is_none());
            assert!(observed.password.is_none());
        }
        for (credentials, user, password) in [
            ("user:password@", b"user".as_slice(), b"password".as_slice()),
            ("user:@", b"user".as_slice(), b"".as_slice()),
            (":password@", b"".as_slice(), b"password".as_slice()),
        ] {
            for authenticate in [false, true] {
                let observed = exchange(
                    "socks5h",
                    "http://destination.invalid/",
                    credentials,
                    authenticate,
                )
                .await;
                assert_eq!(observed.methods, [0, 2]);
                assert!(
                    observed.user.as_deref() == authenticate.then_some(user),
                    "username bytes"
                );
                assert!(
                    observed.password.as_deref() == authenticate.then_some(password),
                    "password bytes"
                );
            }
        }
    });
}

#[test]
fn socks_authentication_uses_utf8_byte_lengths() {
    run(async {
        for (credentials, user, password) in [
            (
                "u%3A%40:p%252B+@".to_owned(),
                b"u:@".to_vec(),
                b"p%2B+".to_vec(),
            ),
            (
                "%C3%A9:%E2%98%83@".to_owned(),
                "é".as_bytes().to_vec(),
                "☃".as_bytes().to_vec(),
            ),
            (
                "u%00ser:p%00ass@".to_owned(),
                b"u\0ser".to_vec(),
                b"p\0ass".to_vec(),
            ),
            (
                format!("{}:{}@", "a".repeat(255), "b".repeat(255)),
                vec![b'a'; 255],
                vec![b'b'; 255],
            ),
        ] {
            let observed =
                exchange("socks5h", "http://destination.invalid/", &credentials, true).await;
            assert!(observed.user == Some(user), "username octets");
            assert!(observed.password == Some(password), "password octets");
        }
    });
}

#[test]
fn socks_unrepresentable_auth_fails_only_if_selected() {
    run(async {
        for credentials in [
            format!("{}:p@", "a".repeat(256)),
            format!("u:{}@", "b".repeat(256)),
        ] {
            let observed =
                exchange("socks", "http://destination.invalid/", &credentials, false).await;
            assert_eq!(observed.methods, [0, 2]);
            let (sent, received) = std::sync::mpsc::channel();
            let server = loopback::start_routed(Arc::new(move |mut socket| {
                use std::io::{Read, Write};
                let mut greeting = [0; 4];
                socket.read_exact(&mut greeting)?;
                assert_eq!(greeting, [5, 2, 0, 2]);
                socket.write_all(&[5, 2])?;
                let mut after = Vec::new();
                socket.read_to_end(&mut after)?;
                sent.send(after).unwrap();
                Ok(())
            }))
            .unwrap();
            let proxy = url::Url::parse(&format!(
                "socks://{credentials}{}",
                server.url.strip_prefix("http://").unwrap()
            ))
            .unwrap();
            let result = sender(proxy)(request("http://destination.invalid/")).await;
            assert!(matches!(
                result,
                Err(crate::providers::http::FetchError::Connection(_))
            ));
            server.finish().unwrap();
            assert!(received.recv().unwrap().is_empty());
        }
    });
}

#[test]
fn socks_connect_keeps_address_families_and_bound_frames() {
    run(async {
        let replies = [
            vec![5, 0, 0, 1, 192, 0, 2, 8, 1, 2],
            vec![5, 0, 0, 3, 5, b'b', b'o', b'u', b'n', b'd', 1, 2],
            [vec![5, 0, 0, 4], vec![7; 16], vec![1, 2]].concat(),
        ];
        for credentials in ["", "user:@"] {
            for reply in &replies {
                check_address_families(credentials, reply).await;
            }
        }
    });
}

/// Keep destination identity independent of the returned bound address.
async fn check_address_families(credentials: &str, reply: &[u8]) {
    for (destination, kind, host) in [
        ("http://127.0.0.2:8082/", 1, vec![127, 0, 0, 2]),
        (
            "http://[::1]:8082/",
            4,
            std::net::Ipv6Addr::LOCALHOST.octets().to_vec(),
        ),
        (
            "http://destination.invalid:8082/",
            3,
            b"destination.invalid".to_vec(),
        ),
    ] {
        let observed = exchange_bound(
            "socks5h",
            destination,
            credentials,
            !credentials.is_empty(),
            reply,
        )
        .await;
        assert_eq!(observed.kind, kind);
        assert_eq!(observed.host, host);
        assert_eq!(observed.port, 8082);
    }
}

/// Recorded rejected protocol exchange.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FailureCase {
    /// Non-secret label.
    id: String,
    /// URL userinfo, empty when absent.
    credentials: String,
    /// Client and peer protocol bytes.
    frames: Vec<socks::Frame>,
    /// Bytes following the failed exchange, recorded through EOF.
    after: Vec<u8>,
}

#[test]
fn socks_handshake_failures_send_no_http_bytes() {
    run(async {
        let cases: Vec<FailureCase> = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/socks_failures.json"
        ))
        .unwrap();
        assert_eq!(cases.len(), 32);
        for case in cases {
            let (sent, received) = std::sync::mpsc::channel();
            let server = loopback::start_routed(Arc::new(move |mut socket| {
                let after = socks::failed_frames(&mut socket, &case.frames)?;
                sent.send(after).unwrap();
                Ok(())
            }))
            .unwrap();
            let userinfo = if case.credentials.is_empty() {
                String::new()
            } else {
                format!("{}@", case.credentials)
            };
            let proxy = url::Url::parse(&format!(
                "socks5h://{userinfo}{}",
                server.url.strip_prefix("http://").unwrap()
            ))
            .unwrap();
            let result = sender(proxy)(request("http://destination.invalid/")).await;
            assert!(
                matches!(
                    result,
                    Err(crate::providers::http::FetchError::Connection(_))
                ),
                "case {}",
                case.id
            );
            server.finish().unwrap();
            assert!(received.recv().unwrap() == case.after, "case {}", case.id);
        }
    });
}

#[test]
fn socks_handshake_preserves_fragmented_and_surplus_bytes() {
    run(async {
        for credentials in ["", "user:@"] {
            for surplus in [false, true] {
                framing_exchange(credentials, surplus).await;
            }
        }
    });
}

/// Independently specified client frames and fragmented peer replies.
fn framing_frames(credentials: &str) -> Vec<socks::Frame> {
    let mut frames = vec![socks::Frame {
        input: if credentials.is_empty() {
            vec![5, 1, 0]
        } else {
            vec![5, 2, 0, 2]
        },
        output: if credentials.is_empty() {
            vec![5, 0]
        } else {
            vec![5, 2]
        },
    }];
    if !credentials.is_empty() {
        frames.push(socks::Frame {
            input: vec![1, 4, b'u', b's', b'e', b'r', 0],
            output: vec![1, 0],
        });
    }
    let mut connect = vec![5, 1, 0, 3, 19];
    connect.extend(b"destination.invalid");
    connect.extend([0, 80]);
    frames.push(socks::Frame {
        input: connect,
        output: vec![5, 0, 0, 1],
    });
    frames
}

/// Distinguish socket surplus preservation from HTTP response ordering.
async fn framing_exchange(credentials: &str, surplus: bool) {
    let frames = framing_frames(credentials);
    let server = loopback::start_routed(Arc::new(move |mut socket| {
        use std::io::Write;
        socks::frames(&mut socket, &frames)?;
        let bound = b"\x7f\0\0\x01\0P";
        let response = b"HTTP/1.1 200 Early\r\ncontent-length: 5\r\nconnection: close\r\n\r\nreply";
        if surplus {
            socket.write_all(&[bound.as_slice(), response].concat())?;
        } else {
            socket.write_all(bound)?;
            assert_eq!(
                loopback::read_request(&mut socket)?.unwrap().body,
                b"payload"
            );
            socket.write_all(response)?;
        }
        Ok(())
    }))
    .unwrap();
    let proxy = url::Url::parse(&format!(
        "socks://{credentials}{}",
        server.url.strip_prefix("http://").unwrap()
    ))
    .unwrap();
    if surplus {
        use tokio::io::AsyncReadExt;
        let mut socket = super::SocksProxyAgent::new(&proxy)
            .unwrap()
            .connect(&"http://destination.invalid/".parse().unwrap())
            .await
            .unwrap();
        let mut bytes = Vec::new();
        socket.read_to_end(&mut bytes).await.unwrap();
        assert_eq!(
            bytes,
            b"HTTP/1.1 200 Early\r\ncontent-length: 5\r\nconnection: close\r\n\r\nreply"
        );
    } else {
        let mut response = sender(proxy)(request("http://destination.invalid/"))
            .await
            .unwrap();
        let mut bytes = Vec::new();
        while let Some(chunk) = response.body.next().await {
            bytes.extend(chunk.unwrap());
        }
        assert_eq!(bytes, b"reply");
    }
    server.finish().unwrap();
}

pub(in crate::providers::http) mod deadlines;

#[test]
fn socks_request_response_conversions_keep_wire_fields() {
    run(async {
        for phrase in ["", "Exact phrase"] {
            let (sent, received) = std::sync::mpsc::channel();
            let server = loopback::start_routed(Arc::new(move |mut socket| {
                use std::io::{Read, Write};
                socks::negotiate(&mut socket, false)?;
                sent.send(loopback::read_request(&mut socket)?.unwrap()).unwrap();
                socket.write_all(format!("HTTP/1.1 302 {phrase}\r\ncontent-length: 3\r\nx-repeat: first\r\nx-repeat: second\r\nlocation: /redirect\r\n\r\n").as_bytes())?;
                socket.write_all(&[0,128,255])?;
                let mut remaining = Vec::new();
                socket.read_to_end(&mut remaining)?;
                assert!(remaining.is_empty());
                Ok(())
            })).unwrap();
            let proxy = url::Url::parse(&server.url.replacen("http", "socks", 1)).unwrap();
            let mut input = request("http://destination.invalid:8081/route?item=2&item=1#discard");
            input.method = "PATCH".into();
            input.body = vec![0, 1, 255];
            let mut response = sender(proxy)(input).await.unwrap();
            assert_eq!(response.status, 302);
            assert_eq!(response.status_text, phrase);
            assert_eq!(response.headers["x-repeat"], "first, second");
            let mut bytes = Vec::new();
            while let Some(chunk) = response.body.next().await {
                bytes.extend(chunk.unwrap());
            }
            assert_eq!(bytes, [0, 128, 255]);
            server.finish().unwrap();
            let observed = received.recv().unwrap();
            assert_eq!(observed.request_line, "PATCH /route?item=2&item=1 HTTP/1.1");
            assert_eq!(observed.header("host"), Some("destination.invalid:8081"));
            assert_eq!(observed.header("accept"), Some("*/*"));
            assert_eq!(observed.header("x-input"), Some("retained"));
            assert_eq!(observed.body, [0, 1, 255]);
        }
        truncated_body_keeps_received_bytes().await;
    });
}

#[test]
fn socks_connection_failure_never_replays_body() {
    run(async {
        let (sent, received) = std::sync::mpsc::channel();
        let server = loopback::start_routed(Arc::new(move |mut socket| {
            socks::negotiate(&mut socket, false)?;
            sent.send(loopback::read_request(&mut socket)?.unwrap().body)
                .unwrap();
            Ok(())
        }))
        .unwrap();
        let proxy = url::Url::parse(&server.url.replacen("http", "socks5h", 1)).unwrap();
        let outcome = sender(proxy)(request("http://destination.invalid/")).await;
        assert!(matches!(
            outcome,
            Err(crate::providers::http::FetchError::Connection(_))
        ));
        server.finish().unwrap();
        let bodies: Vec<_> = received.try_iter().collect();
        assert_eq!(bodies, [b"payload".to_vec()]);
    });
}

#[test]
fn socks_proxy_resolution_errors_close_without_request_bytes() {
    run(async {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        for proxy in ["socks5h://".to_owned(), format!("socks5h://{address}")] {
            let result =
                sender(url::Url::parse(&proxy).unwrap())(request("http://destination.invalid/"))
                    .await;
            assert!(matches!(
                result,
                Err(crate::providers::http::FetchError::Connection(_))
            ));
        }
        let (sent, received) = std::sync::mpsc::channel();
        let server = loopback::serve_closed(move || {
            sent.send(()).unwrap();
        })
        .unwrap();
        let proxy = url::Url::parse(&server.url.replacen("http", "socks5", 1)).unwrap();
        let result = sender(proxy)(request("http://unresolvable.invalid/")).await;
        assert!(matches!(
            result,
            Err(crate::providers::http::FetchError::Connection(_))
        ));
        server.finish().unwrap();
        assert!(received.try_iter().next().is_none());
    });
}

#[test]
fn ipv6_proxy_literal_connects_without_brackets() {
    run(async {
        let server = loopback::start_routed_at(
            "[::1]:0",
            Arc::new(move |mut socket| {
                socks::negotiate(&mut socket, false)?;
                loopback::answer_connection(socket, &|_| loopback::Reply {
                    status: 204,
                    content_type: "text/plain",
                    headers: &[],
                    body: Vec::new(),
                    close: false,
                })
            }),
        )
        .unwrap();
        let proxy = url::Url::parse(&server.url.replacen("http", "socks5h", 1)).unwrap();
        assert_eq!(proxy.host_str(), Some("[::1]"));
        let mut response = sender(proxy.clone())(request("http://destination.invalid/"))
            .await
            .unwrap();
        assert_eq!(response.status, 204);
        assert!(response.body.next().await.is_none());
        server.finish().unwrap();
    });
}

mod tls;

/// Complete authenticated plaintext traffic and return the origin's actual header block.
async fn authenticated_origin_head(scheme: &str) -> Vec<u8> {
    let (sent, received) = std::sync::mpsc::channel();
    let server = loopback::start_routed(Arc::new(move |mut socket| {
        let negotiation = socks::negotiate(&mut socket, true)?;
        assert!(
            negotiation.user.as_deref() == Some(b"proxy-user"),
            "proxy credential octets"
        );
        assert!(
            negotiation.password.as_deref()
                == (!negotiation.methods.is_empty()).then_some(b"proxy-pass".as_slice()),
            "proxy credential octets"
        );
        loopback::answer_connection(socket, &|request| {
            sent.send(request.head.clone()).unwrap();
            loopback::Reply {
                status: 200,
                content_type: "text/plain",
                headers: &[],
                body: Vec::new(),
                close: false,
            }
        })
    }))
    .unwrap();
    let proxy = url::Url::parse(&format!(
        "{scheme}://proxy-user:proxy-pass@{}",
        server.url.strip_prefix("http://").unwrap()
    ))
    .unwrap();
    let mut response = sender(proxy)(request(if scheme == "socks4" {
        "http://localhost/"
    } else {
        "http://destination.invalid/"
    }))
    .await
    .unwrap();
    while let Some(chunk) = response.body.next().await {
        chunk.unwrap();
    }
    server.finish().unwrap();
    received.recv().unwrap()
}

#[test]
fn socks_credentials_do_not_become_origin_headers() {
    run(async {
        for scheme in ["socks5h", "socks4", "socks4a"] {
            let head = authenticated_origin_head(scheme).await;
            assert!(
                !String::from_utf8_lossy(&head)
                    .to_ascii_lowercase()
                    .contains("proxy-authorization")
            );
            assert!(
                !head
                    .windows(10)
                    .any(|bytes| bytes == b"proxy-user" || bytes == b"proxy-pass")
            );
            let encrypted = tls::secure_exchange(Some(scheme), "proxy-user:proxy-pass@").await;
            let proxy = encrypted.proxy.unwrap();
            assert!(
                proxy.user.as_deref() == Some(b"proxy-user"),
                "proxy credential octets"
            );
            assert!(
                proxy.password.as_deref()
                    == (scheme == "socks5h").then_some(b"proxy-pass".as_slice()),
                "proxy credential octets"
            );
            assert!(encrypted.request.header("proxy-authorization").is_none());
            assert!(
                !encrypted
                    .request
                    .head
                    .windows(10)
                    .any(|bytes| bytes == b"proxy-user" || bytes == b"proxy-pass")
            );
        }
    });
}

/// Preserve a received prefix before the native body error for a truncated response.
async fn truncated_body_keeps_received_bytes() {
    let server = loopback::start_routed(Arc::new(move |mut socket| {
        use std::io::Write;
        socks::negotiate(&mut socket, false)?;
        assert!(loopback::read_request(&mut socket)?.is_some());
        socket.write_all(b"HTTP/1.1 200 Prefix\r\ncontent-length: 9\r\n\r\npart")
    }))
    .unwrap();
    let proxy = url::Url::parse(&server.url.replacen("http", "socks5h", 1)).unwrap();
    let mut response = sender(proxy)(request("http://destination.invalid/"))
        .await
        .unwrap();
    let mut bytes = Vec::new();
    loop {
        match response.body.next().await {
            Some(Ok(chunk)) => bytes.extend(chunk),
            Some(Err(crate::providers::http::FetchError::Connection(_))) => break,
            _ => panic!("expected native truncated-body failure"),
        }
    }
    assert_eq!(bytes, b"part");
    server.finish().unwrap();
}

mod socks4;
