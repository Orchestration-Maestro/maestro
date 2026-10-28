//! Real TLS peers distinguish selected transports from fallback-eligible establishment.

use super::{request, routed};
use crate::providers::http::collect_body;
use crate::providers::http::proxy_agent::tests::socks;
use crate::tests::{loopback, tls};
use std::io::{Read, Write};
use std::sync::{Arc, mpsc};

/// Each route stops fallback at a different native connection phase.
#[derive(Clone, Copy, Debug)]
enum Phase {
    /// Plain destination selects at the secure proxy's TCP connection.
    Forward,
    /// Secure destination selects only after the secure proxy's CONNECT exchange.
    Tunnel,
    /// Destination TLS begins after direct selection.
    Direct,
    /// Destination TLS begins after SOCKS negotiation selects the tunnel.
    Socks,
}

#[test]
fn pac_proxy_tls_and_destination_tls_keep_failure_phases() {
    const TEST: &str = "providers::http::client::tests::pac::tls::pac_proxy_tls_and_destination_tls_keep_failure_phases";
    if let Ok(trust) = std::env::var("MAESTRO_PAC_TLS_CHILD") {
        crate::tests::runtime::run(async {
            for phase in [Phase::Forward, Phase::Tunnel, Phase::Direct, Phase::Socks] {
                failure(phase, trust == "trusted").await;
            }
            if trust == "trusted" {
                success(false).await;
                success(true).await;
                connect_failure().await;
                secure_source().await;
            }
        });
        return;
    }
    // Only a verifier that reads a trust file takes `SSL_CERT_FILE`. macOS and Windows verify
    // against the system store, which a test cannot extend, so there the trusted run states
    // its premise instead.
    let system_store = cfg!(any(target_vendor = "apple", windows));
    if system_store {
        eprintln!("the trusted run needs a host whose verifier reads SSL_CERT_FILE");
    }
    let trusts: &[&str] = if system_store {
        &["untrusted"]
    } else {
        &["untrusted", "trusted"]
    };
    for &trust in trusts {
        let process = maestro_test_process::TestProcess::current(TEST)
            .unwrap()
            .env_clear()
            .env("MAESTRO_PAC_TLS_CHILD", trust);
        let process = if trust == "trusted" {
            process.env(
                "SSL_CERT_FILE",
                concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/tests/fixtures/socks-tls/cert.pem"
                ),
            )
        } else {
            process
        };
        let output = process.output().unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

/// A failed certificate handshake must expose no decrypted application bytes.
fn reject_tls(socket: impl Read + Write) {
    let config = tls::server_config(&[b"http/1.1"]).unwrap();
    let mut stream =
        rustls::StreamOwned::new(rustls::ServerConnection::new(config).unwrap(), socket);
    let mut bytes = Vec::new();
    assert!(stream.read_to_end(&mut bytes).is_err());
    assert!(bytes.is_empty());
}

/// Compare certificate failure before or after selection with a completed decoy observation set.
async fn failure(phase: Phase, trust: bool) {
    let (sent, attempts) = mpsc::channel();
    let selected = loopback::start_routed(Arc::new(move |mut socket| {
        sent.send(()).unwrap();
        if matches!(phase, Phase::Socks) {
            socks::negotiate(&mut socket, false)?;
        }
        reject_tls(socket);
        Ok(())
    }))
    .unwrap();
    let (sent, fallbacks) = mpsc::channel();
    let fallback = loopback::serve_routed(move |request| {
        sent.send(request.request_line.clone()).unwrap();
        loopback::Reply {
            status: 407,
            content_type: "text/plain",
            headers: &[],
            body: b"fallback".to_vec(),
            close: false,
        }
    })
    .unwrap();
    let (directive, destination) = selection(phase, trust, &selected.url[7..]);
    let script = format!(
        "function FindProxyForURL(){{return '{directive}; PROXY {}'}}",
        &fallback.url[7..]
    );
    let outcome = routed(&script)(request(destination)).await;
    if matches!(phase, Phase::Tunnel) {
        let response = outcome.unwrap();
        assert_eq!(response.status, 407);
        assert_eq!(collect_body(response.body).await.unwrap(), b"fallback");
    } else {
        let error = outcome.err().unwrap().to_string();
        assert!(error.contains("certificate"), "{phase:?}: {error}");
    }
    selected.finish().unwrap();
    fallback.finish().unwrap();
    assert_eq!(attempts.try_iter().count(), 1);
    let observed = fallbacks.try_iter().collect::<Vec<_>>();
    if matches!(phase, Phase::Tunnel) {
        assert_eq!(observed, ["CONNECT destination.invalid:443 HTTP/1.1"]);
    } else {
        assert!(observed.is_empty(), "{phase:?}: {observed:?}");
    }
}

/// Distinguish wrong-name input from a correctly named but untrusted certificate.
fn selection(phase: Phase, trust: bool, address: &str) -> (String, String) {
    let endpoint = if trust {
        address.to_owned()
    } else {
        address.replacen("127.0.0.1", "localhost", 1)
    };
    match phase {
        Phase::Forward => (
            format!("HTTPS {endpoint}"),
            "http://destination.invalid/".into(),
        ),
        Phase::Tunnel => (
            format!("HTTPS {endpoint}"),
            "https://destination.invalid/".into(),
        ),
        Phase::Direct => ("DIRECT".into(), format!("https://{endpoint}/")),
        Phase::Socks => (
            format!("SOCKS {address}"),
            format!(
                "https://{}/",
                if trust {
                    "wrong.invalid"
                } else {
                    "destination.invalid"
                }
            ),
        ),
    }
}

/// Serve a final encrypted HTTP request with origin-only headers and one body.
fn answer(socket: impl Read + Write, absolute: bool) -> std::io::Result<()> {
    let config = tls::server_config(&[b"http/1.1"]).unwrap();
    let mut stream =
        rustls::StreamOwned::new(rustls::ServerConnection::new(config).unwrap(), socket);
    let request = loopback::read_request(&mut stream)?.unwrap();
    assert_eq!(request.body, b"unique body");
    assert_eq!(
        request.header("proxy-authorization"),
        if absolute { Some("Basic dTpw") } else { None }
    );
    assert_eq!(
        request.request_line,
        if absolute {
            "POST http://destination.invalid/path HTTP/1.1"
        } else {
            "POST /path HTTP/1.1"
        }
    );
    stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")?;
    stream.conn.send_close_notify();
    stream.flush()?;
    let mut rest = Vec::new();
    let _ = stream.read_to_end(&mut rest);
    assert!(rest.is_empty());
    Ok(())
}

/// Verify outer TLS, a successful CONNECT and destination TLS using the shared trust owner.
async fn success(tunnel: bool) {
    let (sent, observed) = mpsc::channel();
    let server = loopback::start_routed(Arc::new(move |socket| {
        if tunnel {
            let config = tls::server_config(&[b"http/1.1"]).unwrap();
            let mut stream =
                rustls::StreamOwned::new(rustls::ServerConnection::new(config).unwrap(), socket);
            let connect = loopback::read_request(&mut stream)?.unwrap();
            assert_eq!(
                connect.request_line,
                "CONNECT destination.invalid:443 HTTP/1.1"
            );
            assert_eq!(connect.header("proxy-authorization"), Some("Basic dTpw"));
            assert!(connect.body.is_empty());
            stream.write_all(b"HTTP/1.1 200 Connection established\r\n\r\n")?;
            stream.flush()?;
            answer(stream, false)?;
        } else {
            answer(socket, true)?;
        }
        sent.send(()).unwrap();
        Ok(())
    }))
    .unwrap();
    let endpoint = server.url[7..].replacen("127.0.0.1", "localhost", 1);
    let script = format!("function FindProxyForURL(){{return 'HTTPS u:p@{endpoint}'}}");
    let destination = format!(
        "{}://destination.invalid/path",
        if tunnel { "https" } else { "http" }
    );
    let response = routed(&script)(request(destination))
        .await
        .unwrap_or_else(|error| panic!("tunnel={tunnel}: {error}"));
    assert_eq!(collect_body(response.body).await.unwrap(), b"ok");
    server.finish().unwrap();
    assert_eq!(observed.try_iter().count(), 1);
}

/// A secure source revalidates only matching metadata and commits changed code with a new context.
async fn secure_source() {
    let (sent, received) = mpsc::channel();
    let count = std::sync::Mutex::new(0);
    let server = loopback::start_routed(Arc::new(move |socket| {
        let config = tls::server_config(&[b"http/1.1"]).unwrap();
        let mut stream = rustls::StreamOwned::new(rustls::ServerConnection::new(config).unwrap(), socket);
        let request = loopback::read_request(&mut stream)?.unwrap();
        let mut count = count.lock().unwrap();
        assert_eq!(request.header("if-none-match"), if *count == 0 { None } else { Some("one") });
        let (status, code) = match *count {
            0 => (200, "let n=0;function FindProxyForURL(){return 'FIRST '+(++n)}"),
            1 => (304, ""),
            2 => (200, "let n=0;function FindProxyForURL(){return 'CHANGED '+(++n)}"),
            _ => panic!("unexpected source load"),
        };
        *count += 1;
        sent.send(()).unwrap();
        write!(stream, "HTTP/1.1 {status} Source\r\nETag: one\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{code}", code.len())?;
        stream.conn.send_close_notify();
        stream.flush()
    })).unwrap();
    let source = url::Url::parse(&server.url.replacen(
        "http://127.0.0.1",
        "pac+https://localhost",
        1,
    ))
    .unwrap();
    let fetch = crate::providers::http::configured_fetch(crate::providers::http::Route {
        proxy: Some(Arc::new(move |_| Ok(Some(source.clone())))),
        ..crate::providers::http::Route::default()
    });
    for expected in ["FIRST 1", "FIRST 2", "CHANGED 1"] {
        let error = fetch(request("http://unused.invalid/".into()))
            .await
            .err()
            .unwrap();
        assert!(error.to_string().contains(expected), "{error}");
    }
    server.finish().unwrap();
    assert_eq!(received.try_iter().count(), 3);
}

/// A trusted proxy closing before CONNECT headers is still an establishment failure.
async fn connect_failure() {
    let selected = loopback::start_routed(Arc::new(move |socket| {
        let config = tls::server_config(&[b"http/1.1"]).unwrap();
        let mut stream =
            rustls::StreamOwned::new(rustls::ServerConnection::new(config).unwrap(), socket);
        assert_eq!(
            loopback::read_request(&mut stream)?.unwrap().request_line,
            "CONNECT destination.invalid:443 HTTP/1.1"
        );
        stream.conn.send_close_notify();
        stream.flush()
    }))
    .unwrap();
    let (sent, received) = mpsc::channel();
    let fallback = loopback::serve_routed(move |_| {
        sent.send(()).unwrap();
        loopback::Reply {
            status: 407,
            content_type: "text/plain",
            headers: &[],
            body: Vec::new(),
            close: false,
        }
    })
    .unwrap();
    let endpoint = selected.url[7..].replacen("127.0.0.1", "localhost", 1);
    let script = format!(
        "function FindProxyForURL(){{return 'HTTPS {endpoint};PROXY {}'}}",
        &fallback.url[7..]
    );
    let response = routed(&script)(request("https://destination.invalid/".into()))
        .await
        .unwrap();
    assert_eq!(response.status, 407);
    collect_body(response.body).await.unwrap();
    selected.finish().unwrap();
    fallback.finish().unwrap();
    assert_eq!(received.try_iter().count(), 1);
}
