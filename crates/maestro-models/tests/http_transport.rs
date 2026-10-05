mod support;
use maestro_models::*;
use std::collections::BTreeMap;
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};
#[tokio::test]
async fn native_transport_posts_exact_bytes_and_returns_streamed_headers() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (release, gate) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut bytes = Vec::new();
        let mut block = [0; 1024];
        loop {
            let n = socket.read(&mut block).await.unwrap();
            bytes.extend_from_slice(&block[..n]);
            if bytes.ends_with(b"literal bytes") {
                break;
            }
        }
        let request = String::from_utf8(bytes).unwrap();
        assert!(request.starts_with("POST /v1/chat/completions HTTP/1.1"));
        assert!(request.contains("authorization: Bearer synthetic"));
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nX-Test: headers\r\n\r\n")
            .await
            .unwrap();
        gate.await.unwrap();
        socket.write_all(b"hello").await.unwrap();
    });
    let native = NativeHttpTransport::new().unwrap();
    let mut response = native
        .send(
            ChatHttpRequest {
                url: format!("http://{address}/v1/chat/completions"),
                headers: BTreeMap::from([("authorization".into(), "Bearer synthetic".into())]),
                body: b"literal bytes".to_vec(),
            },
            600000,
            Cancellation::new(),
        )
        .await
        .unwrap();
    assert_eq!(response.status, 200);
    assert_eq!(response.headers["x-test"], "headers");
    release.send(()).unwrap();
    let mut bytes = Vec::new();
    while let Some(part) = response.body.next().await.unwrap() {
        bytes.extend(part);
    }
    assert_eq!(bytes, b"hello");
    server.await.unwrap();
}
#[tokio::test(start_paused = true)]
async fn native_transport_has_no_hidden_retry_or_body_deadline() {
    let native = NativeHttpTransport::new().unwrap();
    let request = ChatHttpRequest {
        url: "http://127.0.0.1:1/".into(),
        headers: BTreeMap::new(),
        body: vec![],
    };
    assert_eq!(
        native.send(request, 0, Cancellation::new()).await.err(),
        Some(Failure::SetupTimeout)
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (release, gate) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut block = [0; 1024];
        assert!(socket.read(&mut block).await.unwrap() > 0);
        socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\n\r\n")
            .await
            .unwrap();
        gate.await.unwrap();
        socket.write_all(b"x").await.unwrap();
    });
    let mut response = drive(native.send(
        ChatHttpRequest {
            url: format!("http://{address}/"),
            headers: BTreeMap::new(),
            body: vec![],
        },
        600000,
        Cancellation::new(),
    ))
    .await
    .unwrap();
    tokio::time::advance(std::time::Duration::from_secs(1200)).await;
    let counter = std::sync::Arc::new(support::conformance::WakeCounter::default());
    {
        let mut read = response.body.next();
        assert!(support::conformance::poll(read.as_mut(), &counter).is_pending());
    }
    release.send(()).unwrap();
    assert_eq!(response.body.next().await.unwrap(), Some(b"x".to_vec()));
    server.await.unwrap();
    for _ in 0..1000 {
        assert!((0.0..0.25).contains(&native.jitter()));
    }
}
#[test]
fn native_transport_obeys_proxy_environment_in_isolated_processes() {
    use std::io::{Read, Write};
    for case in ["http", "https", "bypass"] {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut bytes = Vec::new();
            let mut buf = [0; 1024];
            while !bytes.windows(4).any(|w| w == b"\r\n\r\n") {
                let n = socket.read(&mut buf).unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&buf[..n]);
            }
            let request = String::from_utf8(bytes).unwrap();
            if case == "https" {
                assert!(request.starts_with("CONNECT example.invalid:443 "));
                socket
                    .write_all(b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\n\r\n")
                    .unwrap();
            } else {
                assert!(request.starts_with(if case == "http" {
                    "POST http://example.invalid/test "
                } else {
                    "POST /test "
                }));
                socket
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                    .unwrap();
            }
        });
        let proxy = format!("http://{address}");
        let url = if case == "bypass" {
            format!("http://{address}/test")
        } else {
            format!("{case}://example.invalid/test")
        };
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "proxy_child", "--ignored"])
            .env_remove("all_proxy")
            .env_remove("ALL_PROXY")
            .env_remove("http_proxy")
            .env_remove("https_proxy")
            .env_remove("no_proxy")
            .env(
                "HTTP_PROXY",
                if case == "bypass" {
                    "http://127.0.0.1:1"
                } else {
                    &proxy
                },
            )
            .env("HTTPS_PROXY", &proxy)
            .env("NO_PROXY", if case == "bypass" { "127.0.0.1" } else { "" })
            .env("MAESTRO_TEST_URL", url)
            .env(
                "MAESTRO_TEST_CONNECT",
                if case == "https" { "yes" } else { "no" },
            )
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        server.join().unwrap();
    }
}
#[tokio::test]
#[ignore = "subprocess helper with isolated proxy environment"]
async fn proxy_child() {
    let native = NativeHttpTransport::new().unwrap();
    let response = native
        .send(
            ChatHttpRequest {
                url: std::env::var("MAESTRO_TEST_URL").unwrap(),
                headers: BTreeMap::new(),
                body: vec![],
            },
            600000,
            Cancellation::new(),
        )
        .await;
    if std::env::var("MAESTRO_TEST_CONNECT").unwrap() == "yes" {
        assert_eq!(response.err(), Some(Failure::Transport));
    } else {
        assert_eq!(response.unwrap().status, 200);
    }
}

async fn drive<F: std::future::Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    std::future::poll_fn(|cx| {
        let result = future.as_mut().poll(cx);
        if result.is_pending() {
            cx.waker().wake_by_ref();
        }
        result
    })
    .await
}
#[tokio::test(start_paused = true)]
async fn native_header_deadline_and_cancellation_use_owned_futures() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let native = NativeHttpTransport::new().unwrap();
    let cancel = Cancellation::new();
    let mut send = native.send(
        ChatHttpRequest {
            url: format!("http://{address}/"),
            headers: BTreeMap::new(),
            body: vec![],
        },
        123,
        cancel.clone(),
    );
    let counter = std::sync::Arc::new(support::conformance::WakeCounter::default());
    let (mut socket, _) = drive(std::future::poll_fn(|cx| {
        assert!(support::conformance::poll(send.as_mut(), &counter).is_pending());
        listener.poll_accept(cx)
    }))
    .await
    .unwrap();
    let mut block = [0; 1024];
    drive(std::future::poll_fn(|cx| {
        assert!(support::conformance::poll(send.as_mut(), &counter).is_pending());
        let mut buffer = tokio::io::ReadBuf::new(&mut block);
        match std::pin::Pin::new(&mut socket).poll_read(cx, &mut buffer) {
            std::task::Poll::Ready(Ok(())) => std::task::Poll::Ready(()),
            std::task::Poll::Ready(Err(e)) => panic!("{e}"),
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }))
    .await;
    tokio::time::advance(std::time::Duration::from_millis(122)).await;
    assert!(support::conformance::poll(send.as_mut(), &counter).is_pending());
    tokio::time::advance(std::time::Duration::from_millis(1)).await;
    assert_eq!(send.await.err(), Some(Failure::SetupTimeout));
    let c = Cancellation::new();
    let mut wait = native.wait(std::time::Duration::MAX, c.clone());
    assert!(support::conformance::poll(wait.as_mut(), &counter).is_pending());
    c.cancel();
    assert_eq!(wait.await, Err(Failure::Cancelled));
}
#[tokio::test(start_paused = true)]
async fn native_wait_retains_total_across_timer_representation_chunks() {
    let native = NativeHttpTransport::new().unwrap();
    let span = std::time::Duration::from_millis((1u64 << 36) - 1);
    let total = span + std::time::Duration::from_millis(17);
    let c = Cancellation::new();
    let counter = std::sync::Arc::new(support::conformance::WakeCounter::default());
    let mut wait = native.wait(total, c);
    assert!(support::conformance::poll(wait.as_mut(), &counter).is_pending());
    tokio::time::advance(span).await;
    assert!(support::conformance::poll(wait.as_mut(), &counter).is_pending());
    tokio::time::advance(std::time::Duration::from_millis(16)).await;
    assert!(support::conformance::poll(wait.as_mut(), &counter).is_pending());
    tokio::time::advance(std::time::Duration::from_millis(1)).await;
    assert_eq!(wait.await, Ok(()));
    let c = Cancellation::new();
    let mut wait = native.wait(total, c.clone());
    assert!(support::conformance::poll(wait.as_mut(), &counter).is_pending());
    tokio::time::advance(span).await;
    assert!(support::conformance::poll(wait.as_mut(), &counter).is_pending());
    c.cancel();
    assert_eq!(wait.await, Err(Failure::Cancelled));
}
#[tokio::test]
async fn native_connection_delivers_replay_and_normalizes_response() {
    for marker in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            let mut block = [0; 1024];
            let body = loop {
                let n = socket.read(&mut block).await.unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&block[..n]);
                if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    let headers = std::str::from_utf8(&bytes[..end])
                        .unwrap()
                        .to_ascii_lowercase();
                    let length: usize = headers
                        .lines()
                        .find_map(|l| l.strip_prefix("content-length: "))
                        .unwrap()
                        .trim()
                        .parse()
                        .unwrap();
                    if bytes.len() >= end + 4 + length {
                        break bytes[end + 4..end + 4 + length].to_vec();
                    }
                }
            };
            let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(
                value["messages"][0]["content"],
                serde_json::json!([{"type":"text","text":"idea"},{"type":"text","text":"answer"}])
            );
            let mut frames = support::chat::success();
            if marker {
                frames.extend(b"data: [DONE]\n\n");
            }
            socket
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n",
                        frames.len()
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
            socket.write_all(&frames).await.unwrap();
        });
        let mut m = support::chat::model();
        m.endpoint = format!("http://{address}/v1");
        let mut d = support::chat::dialect();
        d.thinking_as_text = true;
        let mut models = Models::new(std::sync::Arc::new(|| 73));
        models
            .register(
                m.clone(),
                std::sync::Arc::new(ChatConnection::new(
                    std::sync::Arc::new(NativeHttpTransport::new().unwrap()),
                    d,
                )),
            )
            .unwrap();
        let history = AssistantMessage {
            provider: m.identity.provider.clone(),
            model: m.identity.model.clone(),
            protocol: m.protocol.clone(),
            timestamp: 1,
            content: vec![
                AssistantContent::Thinking(ThinkingContent::Readable {
                    text: "idea".into(),
                    signature: None,
                }),
                AssistantContent::Text(TextContent {
                    text: "answer".into(),
                    replay_metadata: None,
                }),
            ],
            usage: Usage::default(),
            stop_reason: Some(StopReason::Stop),
            failure: None,
            response_id: None,
            response_model: None,
        };
        let out = models
            .complete(
                m,
                Context {
                    system_prompt: None,
                    messages: vec![Message::Assistant(history)],
                    tools: vec![],
                },
                support::auth::local(),
            )
            .await;
        assert_eq!(out.failure, None);
        assert_eq!(out.stop_reason, Some(StopReason::Stop));
        assert_eq!(
            out.content,
            vec![AssistantContent::Text(TextContent {
                text: "hello".into(),
                replay_metadata: None
            })]
        );
        server.await.unwrap();
    }
}
