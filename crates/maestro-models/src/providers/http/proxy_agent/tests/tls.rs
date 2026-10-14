//! Shared TLS trust, server name and origin header witnesses.

use super::run;
use crate::tests::{loopback, socks};
use std::sync::Arc;

/// DER test certificate consumed by both trust and server configuration.
const CERT: &[u8] = include_bytes!("../../../../../tests/fixtures/socks-tls/cert.der");
/// Test-only server key.
const KEY: &[u8] = include_bytes!("../../../../../tests/fixtures/socks-tls/key.der");

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

/// Real TLS server using the same named certificate.
fn server_config() -> Arc<rustls::ServerConfig> {
    let mut config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::aws_lc_rs::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .unwrap()
    .with_no_client_auth()
    .with_single_cert(
        vec![rustls::pki_types::CertificateDer::from(CERT.to_vec())],
        rustls::pki_types::PrivatePkcs8KeyDer::from(KEY.to_vec()).into(),
    )
    .unwrap();
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    Arc::new(config)
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
pub(super) async fn secure_exchange(proxied: bool, credentials: &str) -> Observed {
    let (sent, received) = std::sync::mpsc::channel();
    let config = server_config();
    let authenticate = !credentials.is_empty();
    let authenticate = proxied.then_some(authenticate);
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
    let response = if proxied {
        let proxy = url::Url::parse(&format!(
            "socks5h://{credentials}{}",
            server.url.strip_prefix("http://").unwrap()
        ))
        .unwrap();
        let sender =
            super::super::sender(Arc::new(move |_| Ok(Some(proxy.clone()))), trusted(), None);
        let request = client
            .post("https://destination.invalid/path")
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
        let direct = secure_exchange(false, "").await;
        assert_eq!(direct.name, "localhost");
        assert_eq!(direct.alpn, b"http/1.1");
        let proxied = secure_exchange(true, "").await;
        assert_eq!(proxied.name, "destination.invalid");
        assert_eq!(proxied.alpn, b"http/1.1");
        let proxy = proxied.proxy.unwrap();
        assert_eq!(proxy.host, b"destination.invalid");
        assert_eq!(proxy.port, 443);
        assert_eq!(proxied.request.body, b"payload");
    });
}

#[test]
fn socks_tls_rejects_wrong_name_and_unknown_issuer() {
    run(async {
        for trust in [false, true] {
            let (sent, received) = std::sync::mpsc::channel();
            let config = server_config();
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
                    Err(crate::providers::http::FetchError::Connection(_))
                ));
            }
            server.finish().unwrap();
            assert!(received.recv().unwrap().is_empty());
        }
    });
}
