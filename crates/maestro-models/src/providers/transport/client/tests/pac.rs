//! Script-selected requests use the source consumer before any transport.

use crate::providers::transport::{HttpRequest, Route, collect_body, configured_fetch};

/// Construct a retained route with an authored data source.
fn routed(script: &str) -> crate::Fetch {
    let encoded = percent_encoding::utf8_percent_encode(script, percent_encoding::NON_ALPHANUMERIC);
    let proxy = url::Url::parse(&format!("pac+data:,{encoded}")).unwrap();
    configured_fetch(Route {
        proxy: Some(std::sync::Arc::new(move |_| Ok(Some(proxy.clone())))),
        ..Route::default()
    })
}

/// One distinctive request, with its complete destination preserved.
fn request(url: String) -> HttpRequest {
    HttpRequest {
        method: "POST".into(),
        url,
        headers: indexmap::IndexMap::new(),
        body: b"unique body".to_vec(),
        signal: None,
    }
}

#[test]
fn pac_call_receives_full_url_and_canonical_host() {
    crate::tests::runtime::run(async {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let url = format!("http://{address}/encoded%20path?x=1&x=2");
        let script = format!(
            "function FindProxyForURL(url, host) {{ if (url !== {url:?} || host !== '127.0.0.1') throw Error('lost destination'); return 'DIRECT'; }}"
        );
        let peer = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            while !bytes.ends_with(b"unique body") {
                bytes.push(socket.read_u8().await.unwrap());
            }
            assert!(bytes.starts_with(b"POST /encoded%20path?x=1&x=2 HTTP/1.1\r\n"));
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
                .await
                .unwrap();
        });
        let response = routed(&script)(request(url)).await.unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(collect_body(response.body).await.unwrap(), b"ok");
        peer.await.unwrap();
    });
}

#[test]
fn pac_failed_compilation_never_reaches_direct_transport() {
    crate::tests::runtime::run(async {
        let error = routed("not valid javascript !")(request("http://127.0.0.1:1/".into()))
            .await
            .err()
            .unwrap();
        assert!(error.to_string().contains("SyntaxError"), "{error}");
    });
}

#[test]
fn pac_direct_and_dns_failures_advance_before_application_bytes() {
    crate::tests::runtime::run(async {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let reserved = tokio::net::TcpSocket::new_v4().unwrap();
        reserved.bind("127.0.0.1:0".parse().unwrap()).unwrap();
        let destination = format!("http://{}/once", reserved.local_addr().unwrap());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy = listener.local_addr().unwrap();
        let expected = format!("POST {destination} HTTP/1.1\r\n");
        let peer = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            while !bytes.ends_with(b"unique body") {
                bytes.push(socket.read_u8().await.unwrap());
            }
            assert!(bytes.starts_with(expected.as_bytes()));
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
                .await
                .unwrap();
        });
        let refused = reserved.local_addr().unwrap();
        let script = format!(
            "function FindProxyForURL() {{ return 'DIRECT; PROXY {refused}; PROXY missing.invalid:80; HTTP {proxy}'; }}"
        );
        let response = routed(&script)(request(destination)).await.unwrap();
        assert_eq!(collect_body(response.body).await.unwrap(), b"ok");
        peer.await.unwrap();
    });
}

#[test]
fn pac_tunnel_refusal_never_advances_to_a_later_route() {
    crate::tests::runtime::run(async {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy = listener.local_addr().unwrap();
        let peer = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            while !bytes.ends_with(b"\r\n\r\n") {
                bytes.push(socket.read_u8().await.unwrap());
            }
            let head = String::from_utf8(bytes).unwrap();
            assert!(head.starts_with("CONNECT destination.invalid:8443 HTTP/1.1\r\n"));
            assert!(!head.contains("unique body"));
            socket.write_all(b"HTTP/1.1 407 Proxy Authentication Required\r\nX-Proxy-Result: refused\r\nContent-Length: 7\r\nConnection: close\r\n\r\nrefused").await.unwrap();
            socket.shutdown().await.unwrap();
            let mut leaked = Vec::new();
            socket.read_to_end(&mut leaked).await.unwrap();
            assert!(leaked.is_empty());
        });
        let script = format!("function FindProxyForURL() {{ return 'PROXY {proxy}; DIRECT'; }}");
        let response = routed(&script)(request("https://destination.invalid:8443/secret".into()))
            .await
            .unwrap();
        assert_eq!(response.status, 407);
        assert_eq!(response.status_text, "Proxy Authentication Required");
        assert_eq!(response.headers["x-proxy-result"], "refused");
        assert_eq!(collect_body(response.body).await.unwrap(), b"refused");
        peer.await.unwrap();
    });
}

#[test]
fn pac_results_keep_value_kinds_and_delimiter_rules() {
    crate::tests::runtime::run(async {
        for (value, expected) in [
            ("'  ; A ; ;B; '", r#"["A","B"]"#),
            ("'\\uFEFF X \\uFEFF;\\u0085Y'", "[\"X\",\"\u{85}Y\"]"),
            ("42", r#"["42"]"#),
            ("true", r#"["true"]"#),
            ("['A', 'B']", r#"["A,B"]"#),
            (
                "({toString: function() { return 'CUSTOM'; }})",
                r#"["CUSTOM"]"#,
            ),
            ("' ; ; '", "[]"),
        ] {
            let script = format!("function FindProxyForURL() {{ return {value}; }}");
            let error = routed(&script)(request("http://unused.invalid/".into()))
                .await
                .err()
                .unwrap();
            assert!(error.to_string().ends_with(expected), "{value}: {error}");
        }
        for value in ["undefined", "null", "false", "0", "''"] {
            let server = crate::tests::loopback::serve_routed(|_| crate::tests::loopback::Reply {
                status: 200,
                content_type: "text/plain",
                headers: &[],
                body: b"direct".to_vec(),
                close: false,
            })
            .unwrap();
            let script = format!("function FindProxyForURL(){{return {value}}}");
            let response = routed(&script)(request(server.url.clone())).await.unwrap();
            assert_eq!(collect_body(response.body).await.unwrap(), b"direct");
            server.finish().unwrap();
        }
    });
}

#[test]
fn pac_environment_routes_share_one_source_owner() {
    crate::tests::runtime::run(async {
        let script =
            "var counter = 0; function FindProxyForURL() { return 'SEEN ' + (++counter); }";
        let encoded =
            percent_encoding::utf8_percent_encode(script, percent_encoding::NON_ALPHANUMERIC);
        let source = url::Url::parse(&format!("pac+data:,{encoded}")).unwrap();
        let route = Route {
            proxy: Some(std::sync::Arc::new(move |_| Ok(Some(source.clone())))),
            ..Route::default()
        };
        let first = configured_fetch(route.clone());
        let second = configured_fetch(Route {
            connect_timeout: Some(std::time::Duration::from_secs(7)),
            ..route
        });
        for (fetch, count) in [
            (first.clone(), 1),
            (second, 2),
            (first, 3),
            (routed(script), 1),
        ] {
            let error = fetch(request("http://unused.invalid/".into()))
                .await
                .err()
                .unwrap();
            assert!(
                error.to_string().contains(&format!("SEEN {count}")),
                "{error}"
            );
        }
    });
}

#[test]
fn pac_unknown_candidates_advance_but_invalid_urls_fail() {
    crate::tests::runtime::run(async {
        let unknown = routed("function FindProxyForURL() { return 'unknown; lower case'; }")(
            request("http://unused.invalid/".into()),
        )
        .await
        .err()
        .unwrap();
        assert!(unknown.to_string().ends_with(r#"["unknown","lower case"]"#));
        let malformed = routed("function FindProxyForURL() { return 'PROXY [:80; DIRECT'; }")(
            request("http://unused.invalid/".into()),
        )
        .await
        .err()
        .unwrap();
        assert!(
            !malformed.to_string().contains("Failed to establish"),
            "{malformed}"
        );
        assert!(
            malformed.to_string().contains("invalid IPv6"),
            "{malformed}"
        );
    });
}

#[test]
fn pac_retains_globals_and_replaces_changed_sources() {
    crate::tests::runtime::run(async {
        let path = std::env::temp_dir().join(format!("maestro-pac-retain-{}", std::process::id()));
        let script =
            "var counter = 0; function FindProxyForURL() { return 'INITIAL ' + (++counter); }";
        std::fs::write(&path, script).unwrap();
        let modified = std::fs::metadata(&path).unwrap().modified().unwrap();
        let source =
            url::Url::parse(&format!("pac+{}", url::Url::from_file_path(&path).unwrap())).unwrap();
        let fetch = configured_fetch(Route {
            proxy: Some(std::sync::Arc::new(move |_| Ok(Some(source.clone())))),
            ..Route::default()
        });
        for expected in ["INITIAL 1", "INITIAL 2"] {
            let error = fetch(request("http://unused.invalid/".into()))
                .await
                .err()
                .unwrap();
            assert!(error.to_string().contains(expected), "{error}");
        }
        std::fs::remove_file(&path).unwrap();
        let error = fetch(request("http://unused.invalid/".into()))
            .await
            .err()
            .unwrap();
        assert!(error.to_string().contains("File does not exist"));
        std::fs::write(&path, script).unwrap();
        std::fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(modified)
            .unwrap();
        let error = fetch(request("http://unused.invalid/".into()))
            .await
            .err()
            .unwrap();
        assert!(error.to_string().contains("INITIAL 3"), "{error}");
        std::fs::write(&path, script.replace("INITIAL", "CHANGED")).unwrap();
        std::fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(modified + std::time::Duration::from_secs(1))
            .unwrap();
        let error = fetch(request("http://unused.invalid/".into()))
            .await
            .err()
            .unwrap();
        assert!(error.to_string().contains("CHANGED 1"), "{error}");
        std::fs::remove_file(path).unwrap();
    });
}

#[test]
fn pac_failed_compilation_never_commits_source_cache() {
    crate::tests::runtime::run(failed_compilation("invalid ! code", "SyntaxError"));
}

#[test]
fn pac_missing_callable_never_commits_source_cache() {
    crate::tests::runtime::run(failed_compilation("", "ReferenceError"));
}

#[test]
fn pac_noncallable_export_never_commits_source_cache() {
    crate::tests::runtime::run(failed_compilation("var FindProxyForURL=42", "TypeError"));
}

/// Invalid exports and syntax must not publish conditional source observations.
async fn failed_compilation(code: &str, expected: &str) {
    use tokio::io::AsyncWriteExt;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let source = url::Url::parse(&format!(
        "pac+http://{}/script",
        listener.local_addr().unwrap()
    ))
    .unwrap();
    let code = code.to_owned();
    let peer = tokio::spawn(async move {
        let mut requests = Vec::new();
        let valid = "var n=0; function FindProxyForURL() { return 'VALID ' + (++n); }";
        for (status, code) in [(200, valid), (200, code.as_str()), (304, ""), (200, valid)] {
            let (mut socket, _) = listener.accept().await.unwrap();
            let bytes = source_head(&mut socket).await;
            requests.push(String::from_utf8(bytes).unwrap());
            socket.write_all(format!("HTTP/1.1 {status} Source\r\nETag: one\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{code}", code.len()).as_bytes()).await.unwrap();
        }
        requests
    });
    let fetch = configured_fetch(Route {
        proxy: Some(std::sync::Arc::new(move |_| Ok(Some(source.clone())))),
        ..Route::default()
    });
    for expected in [
        "VALID 1",
        expected,
        "Source has not been modified",
        "VALID 1",
    ] {
        let error = fetch(request("http://unused.invalid/".into()))
            .await
            .err()
            .unwrap();
        assert!(error.to_string().contains(expected), "{error}");
    }
    let requests = peer.await.unwrap();
    assert!(requests[1].contains("if-none-match: one"));
    assert!(!requests[2].contains("if-none-match"));
    assert!(!requests[3].contains("if-none-match"));
}

#[test]
fn pac_concurrent_http_loads_share_only_pending_work() {
    crate::tests::runtime::run(async {
        use tokio::io::AsyncWriteExt;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let source = url::Url::parse(&format!(
            "pac+http://{}/script",
            listener.local_addr().unwrap()
        ))
        .unwrap();
        let peer = tokio::spawn(async move {
            for count in 0..2 {
                let (mut socket, _) = listener.accept().await.unwrap();
                let bytes = source_head(&mut socket).await;
                assert!(!bytes.is_empty());
                let code = "var n=0; function FindProxyForURL() { return 'SHARED ' + (++n); }";
                let response = shared_response(count, bytes, code);
                socket.write_all(response.as_bytes()).await.unwrap();
            }
        });
        let fetch = configured_fetch(Route {
            proxy: Some(std::sync::Arc::new(move |_| Ok(Some(source.clone())))),
            ..Route::default()
        });
        let (one, two) = tokio::join!(
            fetch(request("http://unused.invalid/one".into())),
            fetch(request("http://unused.invalid/two".into()))
        );
        let mut errors = [
            one.err().unwrap().to_string(),
            two.err().unwrap().to_string(),
        ];
        errors.sort();
        assert!(errors[0].contains("SHARED 1"));
        assert!(errors[1].contains("SHARED 2"));
        let third = fetch(request("http://unused.invalid/three".into()))
            .await
            .err()
            .unwrap();
        assert!(third.to_string().contains("SHARED 3"), "{third}");
        peer.await.unwrap();
    });
}

#[test]
fn pac_cancelled_http_source_load_releases_its_socket() {
    crate::tests::runtime::run(async {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let source = url::Url::parse(&format!(
            "pac+http://{}/script",
            listener.local_addr().unwrap()
        ))
        .unwrap();
        let (received, ready) = tokio::sync::oneshot::channel();
        let peer = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            while !bytes.ends_with(b"\r\n\r\n") {
                bytes.push(socket.read_u8().await.unwrap());
            }
            received.send(()).unwrap();
            assert_eq!(socket.read(&mut [0]).await.unwrap(), 0);
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            while !bytes.ends_with(b"\r\n\r\n") {
                bytes.push(socket.read_u8().await.unwrap());
            }
            let code = "function FindProxyForURL() { return 'RECOVERED'; }";
            socket
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{code}",
                        code.len()
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
        });
        let fetch = configured_fetch(Route {
            proxy: Some(std::sync::Arc::new(move |_| Ok(Some(source.clone())))),
            ..Route::default()
        });
        let loading = tokio::spawn(fetch(request("http://unused.invalid/".into())));
        ready.await.unwrap();
        loading.abort();
        assert!(loading.await.err().unwrap().is_cancelled());
        let error = fetch(request("http://unused.invalid/".into()))
            .await
            .err()
            .unwrap();
        assert!(error.to_string().contains("RECOVERED"), "{error}");
        peer.await.unwrap();
    });
}

/// Read the controlled source request before releasing its response.
async fn source_head(socket: &mut tokio::net::TcpStream) -> Vec<u8> {
    use tokio::io::AsyncReadExt;
    let mut bytes = Vec::new();
    while !bytes.ends_with(b"\r\n\r\n") {
        bytes.push(socket.read_u8().await.unwrap());
    }
    bytes
}

/// Revalidation follows the first shared load, never a concurrent duplicate load.
fn shared_response(count: usize, request: Vec<u8>, code: &str) -> String {
    if count == 0 {
        format!(
            "HTTP/1.1 200 OK\r\nETag: one\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{code}",
            code.len()
        )
    } else {
        assert!(
            String::from_utf8(request)
                .unwrap()
                .contains("if-none-match: one")
        );
        "HTTP/1.1 304 Not Modified\r\nConnection: close\r\n\r\n".into()
    }
}

mod callable;
mod deadlines;
mod retention;
mod tls;
mod transport;
