//! SOCKS4 wire observations through the configured sender and retained socket.

use super::{request, run, sender, socks};
use crate::tests::loopback;
use futures_util::StreamExt;
use std::sync::Arc;

/// One projected reference exchange.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Non-secret case label.
    id: String,
    /// Connection inputs.
    input: Input,
    /// Recorded peer and application observations.
    expected: Expected,
}

/// Selected protocol and destination, before URL credential encoding.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Input {
    /// Explicit scheme, otherwise `socks4a`.
    scheme: Option<String>,
    /// Destination spelling.
    host: String,
    /// Decoded identity bytes.
    user: String,
    /// Decoded ignored password.
    #[serde(default)]
    password: String,
    /// Proxy reply status, otherwise granted.
    status: Option<u8>,
    /// Reply prefix size, otherwise eight.
    reply_length: Option<usize>,
    /// Write each reply byte separately.
    #[serde(default)]
    fragmented: bool,
    /// Coalesce reply and following application bytes.
    #[serde(default)]
    surplus: bool,
}

/// Exact wire and completed application result.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    /// Complete client frame, absent only for host-dependent local resolution.
    frame: Option<Vec<u8>>,
    /// Whether negotiation succeeds.
    ok: bool,
    /// Application response bytes on success.
    body: Option<String>,
    /// Application transmission after a refused exchange.
    after: Option<Vec<u8>>,
}

/// Load each row without dropping unknown fields.
fn cases() -> Vec<Case> {
    let cases: Vec<Case> = serde_json::from_str(include_str!(
        "../../../../../tests/fixtures/socks4_wire.json"
    ))
    .unwrap();
    assert_eq!(cases.len(), 65);
    cases
}

/// Encode the selected credentials without changing their decoded bytes.
fn proxy(server: &loopback::Routed, input: &Input) -> url::Url {
    let user =
        percent_encoding::utf8_percent_encode(&input.user, percent_encoding::NON_ALPHANUMERIC);
    let password =
        percent_encoding::utf8_percent_encode(&input.password, percent_encoding::NON_ALPHANUMERIC);
    url::Url::parse(&format!(
        "{}://{user}:{password}@{}",
        input.scheme.as_deref().unwrap_or("socks4a"),
        &server.url[7..]
    ))
    .unwrap()
}

/// Preserve authored IPv6 spelling when constructing the actual request.
fn destination(input: &Input) -> String {
    let host = if input.host.contains(':') {
        format!("[{}]", input.host)
    } else {
        input.host.clone()
    };
    format!("http://{host}:8081/route?first=2&second=1")
}

/// Exchange the complete recorded frame, then inspect real origin traffic.
async fn exchange(case: Case) {
    assert!(case.expected.ok);
    assert_eq!(case.input.status.unwrap_or(90), 90);
    assert_eq!(case.input.reply_length.unwrap_or(8), 8);
    assert!(case.expected.after.is_none());
    let local = case.expected.frame.is_none();
    let frame = case.expected.frame.map(|input| socks::Frame {
        input,
        output: Vec::new(),
    });
    let (sent, received) = std::sync::mpsc::channel();
    let fragmented = case.input.fragmented;
    let surplus = case.input.surplus;
    let expected = case.expected.body.unwrap();
    let mut reply = vec![0, 90, 1, 2, 192, 0, 2, 9];
    if surplus {
        reply.extend(b"surplus");
    }
    let server = loopback::start_routed(Arc::new(move |mut socket| {
        if let Some(frame) = &frame {
            socks::frames(&mut socket, std::slice::from_ref(frame))?;
            write_reply(&mut socket, &reply, fragmented)?;
            if surplus {
                return Ok(());
            }
        } else {
            sent.send(socks::negotiate(&mut socket, false)?).unwrap();
        }
        loopback::answer_connection(socket, &origin_reply)
    }))
    .unwrap();
    let proxy = proxy(&server, &case.input);
    let destination = destination(&case.input);
    let mut body = Vec::new();
    if surplus {
        use tokio::io::AsyncReadExt;
        let mut socket = super::super::SocksProxyAgent::new(&proxy)
            .unwrap()
            .connect(&destination.parse().unwrap())
            .await
            .unwrap();
        socket.read_to_end(&mut body).await.unwrap();
    } else {
        let mut response = sender(proxy)(request(&destination)).await.unwrap();
        while let Some(chunk) = response.body.next().await {
            body.extend(chunk.unwrap());
        }
    }
    server.finish().unwrap();
    if local {
        check_local(received.recv().unwrap(), &case.input).await;
    }
    assert_eq!(body, expected.as_bytes(), "case {}", case.id);
}

/// Send a granted reply either together or byte by byte.
fn write_reply(
    socket: &mut std::net::TcpStream,
    reply: &[u8],
    fragmented: bool,
) -> std::io::Result<()> {
    use std::io::Write;
    if fragmented {
        for byte in reply {
            socket.write_all(&[*byte])?;
        }
    } else {
        socket.write_all(reply)?;
    }
    Ok(())
}

/// Inspect the actual application request after negotiation.
fn origin_reply(request: &loopback::Received) -> loopback::Reply {
    assert_eq!(
        request.request_line,
        "POST /route?first=2&second=1 HTTP/1.1"
    );
    assert_eq!(request.body, b"payload");
    assert!(request.header("proxy-authorization").is_none());
    assert!(request.header("authorization").is_none());
    loopback::Reply {
        status: 200,
        content_type: "text/plain",
        headers: &[],
        body: b"answer".to_vec(),
        close: false,
    }
}

#[test]
fn socks4_user_ids_reach_ipv4_and_domain_peers() {
    run(async {
        let selected: Vec<_> = cases()
            .into_iter()
            .filter(|case| {
                case.id.ends_with("_ipv4")
                    || case.id.ends_with("_ignored_password")
                    || matches!(case.id.as_str(), "empty_domain" | "long_domain_id")
            })
            .collect();
        assert_eq!(selected.len(), 26);
        for case in selected {
            exchange(case).await;
        }
    });
}

#[test]
fn socks4_long_names_and_ids_use_the_same_exchange() {
    run(async {
        let selected: Vec<_> = cases()
            .into_iter()
            .filter(|case| {
                matches!(
                    case.id.as_str(),
                    "buffer_boundary_513" | "buffer_boundary_514" | "long_domain_and_id"
                )
            })
            .collect();
        assert_eq!(selected.len(), 3);
        for (case, size) in selected.into_iter().zip([513, 514, 518]) {
            assert_eq!(case.expected.frame.as_ref().unwrap().len(), size);
            exchange(case).await;
        }
    });
}

#[test]
fn socks4_modes_choose_local_or_proxy_resolution() {
    run(async {
        let selected: Vec<_> = cases()
            .into_iter()
            .filter(|case| {
                case.id.ends_with("_ipv6")
                    || case.id.ends_with("_ipv6_mapped")
                    || case.id.ends_with("_local_or_remote")
            })
            .collect();
        assert_eq!(selected.len(), 6);
        for case in selected {
            exchange(case).await;
        }
        let (sent, received) = std::sync::mpsc::channel();
        let server = loopback::serve_closed(move || {
            sent.send(()).unwrap();
        })
        .unwrap();
        let proxy = url::Url::parse(&server.url.replacen("http", "socks4", 1)).unwrap();
        let outcome = sender(proxy)(request("http://unresolvable.invalid/")).await;
        assert!(matches!(
            outcome,
            Err(crate::providers::http::FetchError::Connection(_))
        ));
        server.finish().unwrap();
        assert!(received.try_iter().next().is_none());
    });
}

/// Assert local DNS membership independently of platform address ordering.
async fn check_local(observed: socks::Negotiation, input: &Input) {
    assert_eq!(input.scheme.as_deref(), Some("socks4"));
    assert!(
        observed.user.as_deref() == Some(input.user.as_bytes()),
        "local identity octets"
    );
    assert!(observed.password.is_none());
    assert_eq!(observed.port, 8081);
    let actual = if observed.kind == 1 {
        std::net::IpAddr::from(<[u8; 4]>::try_from(observed.host).unwrap())
    } else {
        assert_eq!(observed.kind, 3);
        String::from_utf8(observed.host).unwrap().parse().unwrap()
    };
    let addresses: Vec<_> = tokio::net::lookup_host((input.host.as_str(), 8081))
        .await
        .unwrap()
        .map(|address| address.ip())
        .collect();
    assert!(addresses.contains(&actual));
}

#[test]
fn socks4_refusal_and_truncation_send_no_application_bytes() {
    run(async {
        let selected: Vec<_> = cases()
            .into_iter()
            .filter(|case| !case.expected.ok)
            .collect();
        assert_eq!(selected.len(), 26);
        for case in selected {
            refusal(case).await;
        }
    });
}

/// Observe the whole socket after a refusal, not a sampled absence.
async fn refusal(case: Case) {
    assert!(case.expected.body.is_none());
    assert!(!case.input.fragmented && !case.input.surplus);
    let reply = [0, case.input.status.unwrap_or(90), 1, 2, 192, 0, 2, 9];
    let frame = socks::Frame {
        input: case.expected.frame.unwrap(),
        output: reply[..case.input.reply_length.unwrap_or(8)].to_vec(),
    };
    let (sent, received) = std::sync::mpsc::channel();
    let server = loopback::start_routed(Arc::new(move |mut socket| {
        sent.send(socks::failed_frames(
            &mut socket,
            std::slice::from_ref(&frame),
        )?)
        .unwrap();
        Ok(())
    }))
    .unwrap();
    let outcome = sender(proxy(&server, &case.input))(request(&destination(&case.input))).await;
    assert!(
        matches!(
            outcome,
            Err(crate::providers::http::FetchError::Connection(_))
        ),
        "case {}",
        case.id
    );
    server.finish().unwrap();
    assert!(
        received.recv().unwrap() == case.expected.after.unwrap(),
        "case {}",
        case.id
    );
}

#[test]
fn socks4_reply_fragmentation_keeps_surplus_bytes() {
    run(async {
        let selected: Vec<_> = cases()
            .into_iter()
            .filter(|case| case.input.fragmented || case.input.surplus)
            .collect();
        assert_eq!(selected.len(), 4);
        for case in selected {
            exchange(case).await;
        }
    });
}
