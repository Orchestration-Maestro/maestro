//! Retained native authentication for content requests.

use crate::DiagnosticErrorInfo;
use google_cloud_auth::credentials::{CacheableResource, Credentials};
use indexmap::IndexMap;

mod base_external_client;
mod certificate_subject_token_supplier;
mod compute_client;
mod external_account_authorized_user_client;
mod google_auth;
mod identity_pool_client;
mod metadata;
mod oauth_client_auth_handler;
mod sts_credentials;
#[cfg(test)]
mod tests;

/// Scope used for native content generation.
const CLOUD_SCOPE: &str = "https://www.googleapis.com/auth/cloud-platform";
/// Fixed text for any failed credential acquisition.
const ACQUISITION_FAILED: &str = "Authentication: credential acquisition failed";
/// Fixed text for an unusable certificate source.
const CERTIFICATE_FAILED: &str = "Authentication: certificate credentials failed";
/// Fixed text for an unusable credential configuration.
const INVALID_CONFIGURATION: &str = "Authentication: invalid credential configuration";
/// Stable failure when no ADC source is available.
const NO_CREDENTIALS: &str = "Could not load the default credentials. Browse to https://cloud.google.com/docs/authentication/getting-started for more information.";

/// A successful credential owner survives subsequent preparations; failed discovery may retry
/// except for the metadata check, whose result is kept once, including a negative one.
#[derive(Default)]
pub(in crate::providers::content) struct NodeAuth {
    /// The single lazily resolved credential owner.
    credentials: tokio::sync::OnceCell<Credentials>,
}

impl NodeAuth {
    /// Acquire generated headers without content-specific authored fields.
    pub(super) async fn headers(&self) -> Result<IndexMap<String, String>, DiagnosticErrorInfo> {
        let credentials = self
            .credentials
            .get_or_try_init(google_auth::resolve)
            .await?;
        let resource = credentials
            .headers(hyper::http::Extensions::new())
            .await
            .map_err(|error| failure(&error))?;
        let CacheableResource::New { data, .. } = resource else {
            return Err(DiagnosticErrorInfo::error(ACQUISITION_FAILED));
        };
        data.iter()
            .map(|(name, value)| {
                let value = value
                    .to_str()
                    .map_err(|_| DiagnosticErrorInfo::error(ACQUISITION_FAILED))?;
                Ok((name.to_string(), value.to_owned()))
            })
            .collect()
    }
}

/// Fixed safe text, and the HTTP status when one ended the exchange, carried as the source of a
/// credential-library error.
#[derive(Debug)]
struct Cause {
    /// Safe public text.
    message: &'static str,
    /// Final HTTP status of the exchange.
    status: Option<u16>,
}

impl std::fmt::Display for Cause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message)
    }
}

impl std::error::Error for Cause {}

/// A credential-library error whose public text and status are fixed and safe.
fn fixed_error(
    transient: bool,
    message: &'static str,
    status: Option<u16>,
) -> google_cloud_auth::errors::CredentialsError {
    google_cloud_auth::errors::CredentialsError::from_source(transient, Cause { message, status })
}

/// Safe diagnostic keeping the fixed text and HTTP status found in the cause chain.
fn failure(error: &google_cloud_auth::errors::CredentialsError) -> DiagnosticErrorInfo {
    let mut message = ACQUISITION_FAILED;
    let mut status = None;
    let chain = std::iter::successors(std::error::Error::source(error), |next| next.source());
    for next in chain {
        if let Some(cause) = next.downcast_ref::<Cause>() {
            message = cause.message;
            status = cause.status;
            break;
        }
        if let Some(code) = next
            .downcast_ref::<reqwest::Error>()
            .and_then(reqwest::Error::status)
        {
            status = Some(code.as_u16());
            break;
        }
    }
    let name = if error.is_transient() {
        "TransientAuthenticationError"
    } else {
        "AuthenticationError"
    };
    let mut info = DiagnosticErrorInfo::new(Some(name.into()), message);
    info.code = status.map(|status| crate::DiagnosticCode::Number(f64::from(status)));
    info
}
