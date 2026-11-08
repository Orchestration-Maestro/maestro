//! Stored Cloudflare account values with independent environment fallbacks.
use super::{AuthCredential, AuthStorage};
use maestro_models::{
    DiagnosticErrorInfo, Model,
    providers::chat::cloudflare::{is_cloudflare_provider, resolve_cloudflare_base_url_with},
};

impl AuthStorage {
    /// Resolve required Cloudflare account values without executing a key helper.
    ///
    /// Returns no endpoint for other providers. Empty stored values fall back to the environment.
    ///
    /// # Errors
    /// Names a required account or gateway variable that is missing or empty.
    pub fn cloudflare_base_url(
        &self,
        model: &Model,
    ) -> Result<Option<String>, DiagnosticErrorInfo> {
        if !is_cloudflare_provider(&model.provider) {
            return Ok(None);
        }
        let stored = match self.get(&model.provider) {
            Some(AuthCredential::ApiKey(key)) => key.env,
            _ => None,
        };
        let value = |name: &str| {
            stored
                .as_ref()
                .and_then(|env| env.get(name))
                .filter(|value| !value.is_empty())
                .cloned()
                .or_else(|| std::env::var(name).ok().filter(|value| !value.is_empty()))
        };
        for name in ["CLOUDFLARE_ACCOUNT_ID"]
            .into_iter()
            .chain((model.provider == "cloudflare-ai-gateway").then_some("CLOUDFLARE_GATEWAY_ID"))
        {
            if value(name).is_none() {
                return Err(DiagnosticErrorInfo::error(format!(
                    "{name} is required for provider {} but is not set.",
                    model.provider
                )));
            }
        }
        resolve_cloudflare_base_url_with(model, value).map(Some)
    }
}
