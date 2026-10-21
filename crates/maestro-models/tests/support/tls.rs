//! Shared named certificate and controlled TLS server configuration.

#![cfg(not(target_arch = "wasm32"))]
use std::sync::Arc;

/// DER test certificate consumed by trust and server configuration.
pub(crate) const CERT: &[u8] = include_bytes!("../fixtures/socks-tls/cert.der");
/// Test-only server key.
const KEY: &[u8] = include_bytes!("../fixtures/socks-tls/key.der");

/// Build a real TLS server offering the supplied protocols in preference order.
pub(crate) fn server_config(
    protocols: &[&[u8]],
) -> Result<Arc<rustls::ServerConfig>, rustls::Error> {
    let mut config = rustls::ServerConfig::builder_with_provider(Arc::new(
        rustls::crypto::aws_lc_rs::default_provider(),
    ))
    .with_safe_default_protocol_versions()?
    .with_no_client_auth()
    .with_single_cert(
        vec![rustls::pki_types::CertificateDer::from(CERT.to_vec())],
        rustls::pki_types::PrivatePkcs8KeyDer::from(KEY.to_vec()).into(),
    )?;
    config.alpn_protocols = protocols.iter().map(|protocol| protocol.to_vec()).collect();
    Ok(Arc::new(config))
}
