//! Certificate-backed identity-pool refresh over an identity-bearing HTTP effect.

use super::{
    base_external_client::{Effects, Token, TokenSource},
    certificate_subject_token_supplier::{CertificateSubjectTokenSupplier, certificate_error},
    impersonated::Impersonated,
    sts_credentials::StsCredentials,
};
use google_cloud_auth::errors::CredentialsError;
use serde_json::Value;

/// Certificate-source configuration retained by its one shared token cache.
pub(super) struct IdentityPoolClient {
    /// Certificate configuration and trust-chain owner.
    supplier: CertificateSubjectTokenSupplier,
    /// Ordered STS exchange configuration.
    sts: StsCredentials,
    /// Optional final service-account exchange.
    impersonated: Option<Impersonated>,
}

impl IdentityPoolClient {
    /// Select a certificate source without allowing competing file/URL source modes.
    pub(super) fn new(value: &Value) -> Result<Self, CredentialsError> {
        let source = value
            .get("credential_source")
            .ok_or_else(certificate_error)?;
        if ["file", "url"]
            .iter()
            .any(|name| source.get(name).is_some_and(maestro_json::is_truthy))
        {
            return Err(certificate_error());
        }
        let certificate = source.get("certificate").ok_or_else(certificate_error)?;
        Ok(Self {
            supplier: CertificateSubjectTokenSupplier::new(certificate)?,
            sts: StsCredentials::new(value)?,
            impersonated: Impersonated::new(value)?,
        })
    }
}

impl TokenSource for IdentityPoolClient {
    async fn refresh(&self, effects: Effects) -> Result<Token, CredentialsError> {
        let material = self.supplier.material()?;
        let fetch = (effects.identity)(material.identity)?;
        let response = self.sts.exchange(&material.subject, fetch.clone()).await?;
        let token = Token::from_response(&response, (effects.clock)())?;
        if let Some(impersonated) = &self.impersonated {
            impersonated
                .exchange(&token.access, fetch, (effects.clock)())
                .await
        } else {
            Ok(token)
        }
    }
}
