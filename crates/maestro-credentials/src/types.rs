//! Sensitive inputs and fixed safe errors.
use crate::SecretResolver;
use maestro_models::{AuthResolver, SecretString, TokenExchangeResult};
use std::{collections::BTreeMap, sync::Arc};

/// Sensitive caller-supplied stored data; Debug is redacted, not zeroization.
#[derive(Clone, Debug)]
pub enum Credential {
    /// A configured literal, environment name or lazy helper.
    ApiKey {
        /// Sensitive configured value.
        value: SecretString,
    },
    /// Opaque provider token state with supplied request auth and optional expiry.
    Refreshable(TokenExchangeResult),
}
/// Fixed safe failures without paths, parser payloads or external error sources.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CredentialError {
    /// Local waiting or admission was cancelled; admitted writes are not rolled back.
    Cancelled,
    /// The adapter rejects all replacements.
    ReadOnly,
    /// Native ownership remained busy after the permitted attempts.
    Contended,
    /// Stored bytes or the selected record are not the current format.
    Malformed,
    /// Storage access failed.
    Storage,
    /// An explicit location is not an absolute path.
    InvalidPath,
}
impl std::fmt::Display for CredentialError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Cancelled => "credential operation cancelled",
            Self::ReadOnly => "credential storage is read-only",
            Self::Contended => "credential storage is busy",
            Self::Malformed => "malformed credential data",
            Self::Storage => "credential storage failed",
            Self::InvalidPath => "credential location must be absolute",
        })
    }
}
impl std::error::Error for CredentialError {}
/// Injected inputs; names are inert metadata, not secret discovery rules.
pub struct CredentialOptions {
    /// Registered environment names in provider-specific lookup order.
    pub environment_names: BTreeMap<String, Vec<String>>,
    /// Last-priority selected-provider resolver.
    pub fallback: Option<Arc<dyn AuthResolver>>,
    /// Replaceable access to configured secrets.
    pub secrets: Arc<dyn SecretResolver>,
    /// Supplied Unix-millisecond clock for known token expiry.
    pub now: Arc<dyn Fn() -> u64 + Send + Sync>,
}
