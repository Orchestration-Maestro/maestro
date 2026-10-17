//! Certificate configuration, validated leaf material and ordered trust chains.

use super::{CERTIFICATE_FAILED, fixed_error};
use crate::builtins::env_value;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use google_cloud_auth::errors::CredentialsError;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject};
use serde_json::Value;

/// Selected certificate configuration and optional authored trust chain.
pub(super) struct CertificateSubjectTokenSupplier {
    /// Selected configuration path retained across refreshes.
    config: String,
    /// Optional chain file read at each refresh.
    chain: Option<String>,
}

/// Validated subject and identity from the same certificate observation.
pub(super) struct Material {
    /// Exact compact array of base64 DER certificates.
    pub(super) subject: String,
    /// Client identity used only for STS and certificate-owned impersonation.
    pub(super) identity: reqwest::Identity,
}

impl CertificateSubjectTokenSupplier {
    /// Select exactly one certificate configuration mode without reading its material.
    pub(super) fn new(value: &Value) -> Result<Self, CredentialsError> {
        let explicit = value
            .get("certificate_config_location")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty());
        let default = value
            .get("use_default_certificate_config")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if explicit.is_some() == default {
            return Err(certificate_error());
        }
        let config = explicit
            .map(str::to_owned)
            .or_else(|| env_value("GOOGLE_API_CERTIFICATE_CONFIG"))
            .unwrap_or_else(default_path);
        let chain = value
            .get("trust_chain_path")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .map(str::to_owned);
        Ok(Self { config, chain })
    }

    /// Read config, certificate, key and chain afresh at the source refresh boundary.
    pub(super) fn material(&self) -> Result<Material, CredentialsError> {
        if !std::fs::symlink_metadata(&self.config).is_ok_and(|metadata| metadata.is_file()) {
            return Err(certificate_error());
        }
        let config: Value =
            serde_json::from_slice(&read(&self.config)?).map_err(|_| certificate_error())?;
        let workload = config
            .get("cert_configs")
            .and_then(|value| value.get("workload"))
            .ok_or_else(certificate_error)?;
        let cert_path = workload
            .get("cert_path")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or_else(certificate_error)?;
        let key_path = workload
            .get("key_path")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or_else(certificate_error)?;
        let cert = read(cert_path)?;
        let key = read(key_path)?;
        let leaf = CertificateDer::from_pem_slice(&cert).map_err(|_| certificate_error())?;
        validate(&leaf)?;
        PrivateKeyDer::from_pem_slice(&key).map_err(|_| certificate_error())?;
        let subject = self.subject(leaf)?;
        let identity =
            reqwest::Identity::from_pem(&[cert, key].concat()).map_err(|_| certificate_error())?;
        Ok(Material { subject, identity })
    }

    /// Preserve duplicate chain members and reject only a displaced leaf.
    fn subject(&self, leaf: CertificateDer<'static>) -> Result<String, CredentialsError> {
        let mut chain = match &self.chain {
            Some(path) => CertificateDer::pem_slice_iter(&read(path)?)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| certificate_error())?,
            None => Vec::new(),
        };
        for cert in &chain {
            validate(cert)?;
        }
        match chain.iter().position(|cert| cert == &leaf) {
            None => chain.insert(0, leaf),
            Some(0) => {}
            Some(_) => return Err(certificate_error()),
        }
        let encoded: Vec<_> = chain
            .iter()
            .map(|cert| STANDARD.encode(cert.as_ref()))
            .collect();
        maestro_json::compact_json(&serde_json::json!(encoded)).map_err(|_| certificate_error())
    }
}

/// Platform gcloud default; unlike ADC, a missing home uses a relative default.
fn default_path() -> String {
    let root = env_value("CLOUDSDK_CONFIG").unwrap_or_else(|| {
        if cfg!(windows) {
            maestro_path::join(&[&env_value("APPDATA").unwrap_or_default(), "gcloud"])
        } else {
            maestro_path::join(&[&env_value("HOME").unwrap_or_default(), ".config", "gcloud"])
        }
    });
    maestro_path::join(&[&root, "certificate_config.json"])
}

/// Read without exposing authored paths or file contents in diagnostics.
fn read(path: &str) -> Result<Vec<u8>, CredentialsError> {
    std::fs::read(path).map_err(|_| certificate_error())
}

/// Validate DER using the existing native certificate parser.
fn validate(cert: &CertificateDer<'_>) -> Result<(), CredentialsError> {
    rustls::server::ParsedCertificate::try_from(cert)
        .map(|_| ())
        .map_err(|_| certificate_error())
}

/// Safe certificate failure without parser or key text.
pub(super) fn certificate_error() -> CredentialsError {
    fixed_error(false, CERTIFICATE_FAILED, None)
}
