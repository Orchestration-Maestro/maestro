//! Shared TLS trust, server name and origin header witnesses.

use super::{run, socks};
use crate::tests::{
    loopback,
    tls::{CERT, server_config},
};
use std::sync::Arc;

/// One controlled issuer without weakening name or chain verification.
fn trusted() -> rustls::ClientConfig {
    let mut roots = rustls::RootCertStore::empty();
    roots
        .add(rustls::pki_types::CertificateDer::from(CERT.to_vec()))
        .unwrap();
    let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
    let mut config = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_root_certificates(roots)
        .with_no_client_auth();
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    config
}

/// Observation of a completed encrypted request.
pub(super) struct Observed {
    /// Negotiated destination name.
    name: String,
    /// Selected protocol.
    alpn: Vec<u8>,
    /// Decrypted request.
    pub(super) request: loopback::Received,
    /// Proxy negotiation, when used.
    pub(super) proxy: Option<socks::Negotiation>,
}

/// Serve one encrypted request, observing name and protocol only after TLS completes.
fn secure_peer(
    mut socket: std::net::TcpStream,
    config: &Arc<rustls::ServerConfig>,
    authenticate: Option<bool>,
    sent: &std::sync::mpsc::Sender<Observed>,
) -> std::io::Result<()> {
    use std::io::Write;
    let proxy = if let Some(authenticate) = authenticate {
        Some(socks::negotiate(&mut socket, authenticate)?)
    } else {
        None
    };
    let mut tls = rustls::StreamOwned::new(
        rustls::ServerConnection::new(Arc::clone(config)).unwrap(),
        socket,
    );
    let request = loopback::read_request(&mut tls)?.unwrap();
    sent.send(Observed {
        name: tls.conn.server_name().unwrap().to_owned(),
        alpn: tls.conn.alpn_protocol().unwrap().to_vec(),
        request,
        proxy,
    })
    .unwrap();
    tls.write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 5\r\nconnection: close\r\n\r\nreply")?;
    tls.conn.send_close_notify();
    tls.flush()
}

/// Complete a real TLS request through either sender with one supplied trust configuration.
pub(super) async fn secure_exchange(scheme: Option<&str>, credentials: &str) -> Observed {
    let (sent, received) = std::sync::mpsc::channel();
    let config = server_config(&[b"http/1.1"]).unwrap();
    let authenticate = !credentials.is_empty();
    let authenticate = scheme.map(|_| authenticate);
    let server = loopback::start_routed(Arc::new(move |socket| {
        secure_peer(socket, &config, authenticate, &sent)
    }))
    .unwrap();
    let client = reqwest::Client::builder()
        .no_proxy()
        .http1_only()
        .tls_backend_preconfigured(trusted())
        .build()
        .unwrap();
    let response = if let Some(scheme) = scheme {
        let proxy = url::Url::parse(&format!(
            "{scheme}://{credentials}{}",
            server.url.strip_prefix("http://").unwrap()
        ))
        .unwrap();
        let sender =
            super::super::sender(Arc::new(move |_| Ok(Some(proxy.clone()))), trusted(), None);
        let request = client
            .post(if scheme == "socks4" {
                "https://localhost/path"
            } else {
                "https://destination.invalid/path"
            })
            .body(b"payload".to_vec())
            .build()
            .unwrap();
        reqwest::Response::from(sender.request(request.try_into().unwrap()).await.unwrap())
    } else {
        client
            .post(
                server
                    .url
                    .replacen("http://127.0.0.1", "https://localhost", 1),
            )
            .body(b"payload".to_vec())
            .send()
            .await
            .unwrap()
    };
    assert_eq!(response.bytes().await.unwrap().as_ref(), b"reply");
    server.finish().unwrap();
    received.recv().unwrap()
}

#[test]
fn socks_tls_uses_shared_trust_and_destination_name() {
    run(async {
        let direct = secure_exchange(None, "").await;
        assert_eq!(direct.name, "localhost");
        assert_eq!(direct.alpn, b"http/1.1");
        for (scheme, credentials) in [
            ("socks5h", String::new()),
            ("socks4a", String::new()),
            ("socks4a", format!("{}@", "x".repeat(256))),
        ] {
            let proxied = secure_exchange(Some(scheme), &credentials).await;
            assert_eq!(proxied.name, "destination.invalid");
            assert_eq!(proxied.alpn, b"http/1.1");
            let proxy = proxied.proxy.unwrap();
            assert_eq!(proxy.host, b"destination.invalid");
            assert_eq!(proxy.port, 443);
            assert_eq!(proxied.request.body, b"payload");
            if scheme == "socks4a" {
                let expected = credentials.strip_suffix('@').unwrap_or("");
                assert!(
                    proxy.user.as_deref() == Some(expected.as_bytes()),
                    "proxy identity bytes"
                );
                assert!(proxy.password.is_none());
            }
        }
    });
}

#[test]
fn socks_tls_rejects_wrong_name_and_unknown_issuer() {
    run(async {
        for trust in [false, true] {
            let (sent, received) = std::sync::mpsc::channel();
            let config = server_config(&[b"http/1.1"]).unwrap();
            let server = loopback::start_routed(Arc::new(move |mut socket| {
                use std::io::Read;
                socks::negotiate(&mut socket, false)?;
                let mut tls = rustls::StreamOwned::new(
                    rustls::ServerConnection::new(Arc::clone(&config)).unwrap(),
                    socket,
                );
                let mut application = Vec::new();
                let _ = tls.read_to_end(&mut application);
                sent.send(application).unwrap();
                Ok(())
            }))
            .unwrap();
            let proxy = url::Url::parse(&server.url.replacen("http", "socks5h", 1)).unwrap();
            if trust {
                let sender = super::super::sender(
                    Arc::new(move |_| Ok(Some(proxy.clone()))),
                    trusted(),
                    None,
                );
                let request = reqwest::Client::new()
                    .post("https://wrong.invalid/")
                    .body(b"not-sent".to_vec())
                    .build()
                    .unwrap();
                assert!(sender.request(request.try_into().unwrap()).await.is_err());
            } else {
                let outcome =
                    super::sender(proxy)(super::request("https://destination.invalid/")).await;
                assert!(matches!(
                    outcome,
                    Err(crate::providers::transport::FetchError::Connection(_))
                ));
            }
            server.finish().unwrap();
            assert!(received.recv().unwrap().is_empty());
        }
    });
}
