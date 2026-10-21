//! Public cloud conversations over controlled SOCKS transports.
#![cfg(not(target_arch = "wasm32"))]

use crate::{bedrock, chat, loopback, socks, transport};

use chat::TestResult;
use maestro_models::providers::cloud::amazon_bedrock::{
    BedrockOptions, stream_bedrock, stream_simple_bedrock,
};
use maestro_models::{SimpleStreamOptions, StreamOptions};
use std::sync::Arc;

/// Drive either public caller in its isolated child environment.
async fn conversation(
    common: StreamOptions,
) -> TestResult<(Vec<String>, maestro_models::AssistantMessage)> {
    let model = bedrock::model(&std::env::var("MAESTRO_ORIGIN")?)?;
    let context = bedrock::context("question")?;
    let stream = if std::env::var_os("MAESTRO_SIMPLE").is_some() {
        stream_simple_bedrock(
            model,
            context,
            Some(SimpleStreamOptions {
                common,
                ..SimpleStreamOptions::default()
            }),
        )
    } else {
        stream_bedrock(
            model,
            context,
            Some(BedrockOptions {
                common,
                ..BedrockOptions::default()
            }),
        )
    };
    transport::drain(&stream).await
}

/// Require the complete conversation rather than an isolated route decision.
fn completed(labels: &[String], message: &maestro_models::AssistantMessage) -> TestResult {
    assert_eq!(labels.last().map(String::as_str), Some("done"));
    assert!(message.error_message.is_none());
    assert_eq!(serde_json::to_value(&message.content)?[0]["text"], "answer");
    Ok(())
}

/// Destination identity and the complete origin-form request line.
type Destination = (u8, Vec<u8>, u16, String);

/// Reply with real cloud frames after SOCKS negotiation and HTTP parsing.
fn converse_peer(
    mut socket: std::net::TcpStream,
    body: &[u8],
    sent: &std::sync::mpsc::Sender<Destination>,
    identity: Option<&[u8]>,
) -> std::io::Result<()> {
    let (proxy, ()) =
        socks::negotiate_observing(&mut socket, false, socks::DEFAULT_REPLY, |_, user, _| {
            assert!(user.as_deref() == identity, "proxy identity bytes");
        })?;
    loopback::answer_connection(socket, &|request| {
        assert!(
            sent.send((
                proxy.kind,
                proxy.host.clone(),
                proxy.port,
                request.request_line.clone(),
            ))
            .is_ok()
        );
        loopback::Reply {
            status: 200,
            content_type: "application/vnd.amazon.eventstream",
            headers: &[],
            body: body.to_vec(),
            close: false,
        }
    })
}

#[test]
fn bedrock_socks_routes_reach_conversation_stream() -> TestResult {
    if maestro_test_process::selection().is_child() {
        return crate::runtime::run(async {
            let (labels, message) = conversation(StreamOptions::default()).await?;
            completed(&labels, &message)
        });
    }
    for scheme in ["socks", "socks5", "socks5h", "socks4", "socks4a"] {
        let identities = if scheme.starts_with("socks4") {
            vec![String::new(), "u%3A%40".to_owned(), "x".repeat(256)]
        } else {
            vec![String::new()]
        };
        for identity in identities {
            for simple in [false, true] {
                routed_conversation(scheme, &identity, simple)?;
            }
        }
    }
    Ok(())
}

/// Observe one public caller through one selected identity and protocol.
fn routed_conversation(scheme: &str, identity: &str, simple: bool) -> TestResult {
    let body = bedrock::answer("answer")?.concat();
    let (sent, received) = std::sync::mpsc::channel();
    let expected = scheme
        .starts_with("socks4")
        .then(|| percent_encoding::percent_decode_str(identity).collect::<Vec<_>>());
    let server = loopback::start_routed(Arc::new(move |socket| {
        converse_peer(socket, &body, &sent, expected.as_deref())
    }))?;
    let proxy = if scheme.starts_with("socks4") {
        format!("{scheme}://{identity}:ignored@{}", &server.url[7..])
    } else {
        server.url.replacen("http", scheme, 1)
    };
    let origin = if matches!(scheme, "socks5" | "socks4") {
        "http://localhost:8081"
    } else {
        "http://destination.invalid:8081"
    };
    let mut vars = vec![
        ("HTTP_PROXY", proxy.as_str()),
        ("MAESTRO_ORIGIN", origin),
        ("AWS_REGION", "us-east-1"),
        ("AWS_BEARER_TOKEN_BEDROCK", "controlled-bearer"),
        ("AWS_MAX_ATTEMPTS", "1"),
    ];
    if simple {
        vars.push(("MAESTRO_SIMPLE", "1"));
    }
    let result = maestro_test_process::TestProcess::current(
        "bedrock_socks_routes_reach_conversation_stream",
    )
    .and_then(|process| {
        process
            .nocapture()
            .env_clear()
            .envs(vars.iter().copied())
            .case(scheme)
            .output()
    });
    server.finish()?;
    let output = result?;
    assert!(output.status.success(), "{output:?}");
    check_destination(scheme, &received.try_iter().collect::<Vec<_>>())?;
    Ok(())
}

/// Inspect the destination identity and origin-form request after peer completion.
fn check_destination(scheme: &str, observations: &[Destination]) -> TestResult {
    assert_eq!(observations.len(), 1);
    let (kind, host, port, request) = &observations[0];
    if matches!(scheme, "socks5" | "socks4") {
        let addresses = std::net::ToSocketAddrs::to_socket_addrs(&("localhost", 8081))?;
        assert!(
            addresses
                .into_iter()
                .any(|address| match (*kind, address.ip()) {
                    (1, std::net::IpAddr::V4(ip)) => host.as_slice() == ip.octets(),
                    (4, std::net::IpAddr::V6(ip)) => host.as_slice() == ip.octets(),
                    (3, ip) if scheme == "socks4" => host.as_slice() == ip.to_string().as_bytes(),
                    _ => false,
                })
        );
    } else {
        assert_eq!(*kind, 3);
        assert_eq!(host, b"destination.invalid");
    }
    assert_eq!(*port, 8081);
    assert_eq!(
        request,
        "POST /model/meta.llama-controlled/converse-stream HTTP/1.1"
    );
    Ok(())
}

/// Abort only after the peer acknowledges the chosen connection phase.
async fn cancel_conversation() -> TestResult {
    let signal = maestro_models::Cancellation::new();
    let common = StreamOptions {
        signal: Some(signal.clone()),
        ..StreamOptions::default()
    };
    let cancel = async {
        reqwest::Client::builder()
            .no_proxy()
            .build()?
            .get(std::env::var("MAESTRO_GATE")?)
            .send()
            .await?;
        signal.abort();
        Ok::<_, Box<dyn std::error::Error>>(())
    };
    let (result, cancelled) = futures_util::future::join(conversation(common), cancel).await;
    cancelled?;
    let (labels, message) = result?;
    assert_eq!(message.stop_reason, maestro_models::StopReason::Aborted);
    assert_eq!(labels.iter().filter(|label| *label == "error").count(), 1);
    assert_eq!(labels.last().map(String::as_str), Some("error"));
    Ok(())
}

/// Record the request phase and observe closure of that same socket.
fn cancellation_peer(
    mut socket: std::net::TcpStream,
    after_request: bool,
    ready: &std::sync::mpsc::Sender<()>,
    sent: &std::sync::mpsc::Sender<(Vec<u8>, Vec<u8>)>,
    identity: Option<&[u8]>,
) -> std::io::Result<()> {
    use std::io::Read;
    let body = if after_request {
        socks::negotiate_observing(&mut socket, false, socks::DEFAULT_REPLY, |_, user, _| {
            assert!(user.as_deref() == identity, "proxy identity bytes");
        })?;
        loopback::read_request(&mut socket)?
            .ok_or_else(|| std::io::Error::other("missing request"))?
            .body
    } else {
        let mut version = [0];
        socket.read_exact(&mut version)?;
        if version == [4] {
            socks::read4(&mut socket, |_, user, _| {
                assert!(user.as_deref() == identity, "proxy identity bytes");
            })?;
        } else {
            assert_eq!(version, [5]);
            let mut hello = [0; 2];
            socket.read_exact(&mut hello)?;
            assert_eq!(hello, [1, 0]);
        }
        Vec::new()
    };
    ready.send(()).map_err(std::io::Error::other)?;
    let mut remaining = Vec::new();
    socket.read_to_end(&mut remaining)?;
    sent.send((body, remaining))
        .map_err(std::io::Error::other)?;
    Ok(())
}

#[test]
fn socks_cancel_closes_inflight_connection() -> TestResult {
    if maestro_test_process::selection().is_child() {
        return crate::runtime::run(cancel_conversation());
    }
    if maestro_test_process::run_alone("socks_cancel_closes_inflight_connection")? {
        return Ok(());
    }
    for (scheme, identity) in [
        ("socks5h", None),
        ("socks4a", Some(String::new())),
        ("socks4a", Some("x".repeat(256))),
    ] {
        for after_request in [false, true] {
            cancellation_case(scheme, identity.as_deref(), after_request)?;
        }
    }
    Ok(())
}

/// Drive one cancellation boundary, then join both controlled peers.
fn cancellation_case(scheme: &str, identity: Option<&str>, after_request: bool) -> TestResult {
    let (ready, gate) = std::sync::mpsc::channel();
    let (sent, received) = std::sync::mpsc::channel();
    let expected = identity.map(str::to_owned);
    let server = loopback::start_routed(Arc::new(move |socket| {
        cancellation_peer(
            socket,
            after_request,
            &ready,
            &sent,
            expected.as_deref().map(str::as_bytes),
        )
    }))?;
    let gate_server = cancellation_gate(gate)?;
    let proxy = identity.map_or_else(
        || server.url.replacen("http", scheme, 1),
        |user| format!("{scheme}://{user}@{}", &server.url[7..]),
    );
    let vars = [
        ("HTTP_PROXY", proxy.as_str()),
        ("MAESTRO_ORIGIN", "http://destination.invalid"),
        ("MAESTRO_GATE", gate_server.url.as_str()),
        ("AWS_REGION", "us-east-1"),
        ("AWS_BEARER_TOKEN_BEDROCK", "controlled-bearer"),
        ("AWS_MAX_ATTEMPTS", "1"),
    ];
    let result =
        maestro_test_process::TestProcess::current("socks_cancel_closes_inflight_connection")
            .and_then(|process| {
                process
                    .nocapture()
                    .env_clear()
                    .envs(vars.iter().copied())
                    .case(if after_request {
                        "request"
                    } else {
                        "handshake"
                    })
                    .output()
            });
    server.finish()?;
    gate_server.finish()?;
    let output = result?;
    assert!(output.status.success(), "{output:?}");
    let observations: Vec<_> = received.try_iter().collect();
    assert_eq!(observations.len(), 1);
    assert_eq!(!observations[0].0.is_empty(), after_request);
    assert!(observations[0].1.is_empty());
    Ok(())
}

/// Acknowledge only after the selected SOCKS socket reaches its boundary.
fn cancellation_gate(gate: std::sync::mpsc::Receiver<()>) -> TestResult<loopback::Routed> {
    let gate = std::sync::Mutex::new(gate);
    let gate_server = loopback::serve_routed(move |_| {
        assert!(
            gate.lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .recv()
                .is_ok()
        );
        loopback::Reply {
            status: 200,
            content_type: "text/plain",
            headers: &[],
            body: Vec::new(),
            close: false,
        }
    })?;
    Ok(gate_server)
}

/// Observe failure, bypass or replacement through the public conversation caller.
async fn bypass_conversation(case: &str) -> TestResult {
    let scripted = bedrock::serve(bedrock::answer("answer")?);
    let common = StreamOptions {
        fetch: (case == "replacement").then(|| Arc::clone(&scripted.fetch)),
        ..StreamOptions::default()
    };
    let (labels, message) = conversation(common).await?;
    if case == "selected" {
        assert!(message.error_message.is_some());
        assert_eq!(labels.last().map(String::as_str), Some("error"));
    } else {
        completed(&labels, &message)?;
    }
    if case == "replacement" {
        assert_eq!(scripted.seen()?.len(), 1);
    }
    Ok(())
}

#[test]
fn bedrock_socks_bypass_and_replacement_avoid_invalid_proxy() -> TestResult {
    let case = maestro_test_process::selection();
    if case.is_child() {
        let case = ["selected", "bypass", "replacement"]
            .into_iter()
            .find(|id| case.runs_case(id))
            .ok_or("unknown child case")?;
        return crate::runtime::run(bypass_conversation(case));
    }
    for scheme in ["socks5h", "socks4", "socks4a"] {
        for case in ["selected", "bypass", "replacement"] {
            let body = bedrock::answer("answer")?.concat();
            let (sent, received) = std::sync::mpsc::channel();
            let origin = loopback::serve_routed(move |request| {
                sent.send(request.request_line.clone()).unwrap();
                loopback::Reply {
                    status: 200,
                    content_type: "application/vnd.amazon.eventstream",
                    headers: &[],
                    body: body.clone(),
                    close: false,
                }
            })?;
            let proxy = format!("{scheme}://bad%ZZ@unresolvable.invalid");
            let mut vars = vec![
                ("HTTP_PROXY", proxy.as_str()),
                ("MAESTRO_ORIGIN", origin.url.as_str()),
                ("AWS_REGION", "us-east-1"),
                ("AWS_BEARER_TOKEN_BEDROCK", "controlled-bearer"),
                ("AWS_MAX_ATTEMPTS", "1"),
            ];
            if case == "bypass" {
                vars.push(("NO_PROXY", "*"));
            }
            let result = maestro_test_process::TestProcess::current(
                "bedrock_socks_bypass_and_replacement_avoid_invalid_proxy",
            )
            .and_then(|process| {
                process
                    .nocapture()
                    .env_clear()
                    .envs(vars.iter().copied())
                    .case(case)
                    .output()
            });
            origin.finish()?;
            let output = result?;
            assert!(output.status.success(), "{output:?}");
            let requests: Vec<_> = received.try_iter().collect();
            if case == "bypass" {
                assert_eq!(
                    requests,
                    ["POST /model/meta.llama-controlled/converse-stream HTTP/1.1"]
                );
            } else {
                assert!(requests.is_empty());
            }
        }
    }
    Ok(())
}
