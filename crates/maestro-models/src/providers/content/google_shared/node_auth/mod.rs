//! Retained native authentication for content requests.

use crate::DiagnosticErrorInfo;
use google_cloud_auth::credentials::{CacheableResource, Credentials};
use indexmap::IndexMap;

mod base_external_client;
mod external_account_authorized_user_client;
mod google_auth;
mod oauth_client_auth_handler;

/// Scope used for native content generation.
const CLOUD_SCOPE: &str = "https://www.googleapis.com/auth/cloud-platform";
/// Stable failure when no ADC source is available.
const NO_CREDENTIALS: &str = "Could not load the default credentials. Browse to https://cloud.google.com/docs/authentication/getting-started for more information.";

/// A successful credential owner survives subsequent preparations; failed discovery may retry.
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
            return Err(DiagnosticErrorInfo::error(
                "Authentication: credential acquisition failed",
            ));
        };
        data.iter()
            .map(|(name, value)| {
                let value = value.to_str().map_err(|_| {
                    DiagnosticErrorInfo::error("Authentication: credential acquisition failed")
                })?;
                Ok((name.to_string(), value.to_owned()))
            })
            .collect()
    }
}

/// Safe diagnostic keeping only the stage, transience and any HTTP status in the cause chain.
fn failure(error: &google_cloud_auth::errors::CredentialsError) -> DiagnosticErrorInfo {
    let mut info = DiagnosticErrorInfo::new(
        Some(
            if error.is_transient() {
                "TransientAuthenticationError"
            } else {
                "AuthenticationError"
            }
            .into(),
        ),
        "Authentication: credential acquisition failed",
    );
    let mut cause = std::error::Error::source(error);
    while let Some(next) = cause {
        let status = next
            .downcast_ref::<oauth_client_auth_handler::Status>()
            .map(|status| status.0)
            .or_else(|| {
                next.downcast_ref::<reqwest::Error>()?
                    .status()
                    .map(|s| s.as_u16())
            });
        if let Some(status) = status {
            info.code = Some(crate::DiagnosticCode::Number(f64::from(status)));
            break;
        }
        cause = next.source();
    }
    info
}

#[cfg(test)]
mod tests;

mod certificate_subject_token_supplier;
mod compute_client;
mod identity_pool_client;
mod impersonated;
mod metadata;
mod sts_credentials;
