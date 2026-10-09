//! Device-account authorization and model endpoint selection.
mod polling;
mod token;

use crate::oauth::authorization_whitespace;
use crate::{
    BoxFuture, Fetch, Model, OAuthAuthInfo, OAuthCallbacks, OAuthCredentials, OAuthError,
    OAuthPrompt, OAuthProviderInterface, default_fetch,
};
use serde::Deserialize as _;
use url::Url;

/// Parse an enterprise URL or domain into its hostname, retaining valid empty hosts.
#[must_use]
pub fn normalize_domain(input: &str) -> Option<String> {
    let trimmed = input.trim_matches(authorization_whitespace);
    if trimmed.is_empty() {
        return None;
    }
    let url = if trimmed.contains("://") {
        Url::parse(trimmed)
    } else {
        Url::parse(&format!("https://{trimmed}"))
    };
    url.ok()
        .map(|url| url.host_str().unwrap_or_default().to_owned())
}

/// Choose token endpoint text before a nonempty enterprise domain or the standard endpoint.
#[must_use]
pub fn get_github_copilot_base_url(token: Option<&str>, enterprise_domain: Option<&str>) -> String {
    if let Some(host) = token.and_then(|token| {
        token
            .match_indices("proxy-ep=")
            .find_map(|(offset, matched)| {
                let tail = &token[offset + matched.len()..];
                let host = tail.split(';').next().unwrap_or_default();
                (!host.is_empty()).then_some(host)
            })
    }) {
        return match host.strip_prefix("proxy.") {
            Some(suffix) => format!("https://api.{suffix}"),
            None => format!("https://{host}"),
        };
    }
    enterprise_domain
        .filter(|domain| !domain.is_empty())
        .map_or_else(
            || "https://api.individual.githubcopilot.com".into(),
            |domain| format!("https://copilot-api.{domain}"),
        )
}

/// Exchange a device-account refresh token without storing credentials.
pub fn refresh_github_copilot_token(
    refresh_token: String,
    enterprise_domain: Option<String>,
    fetch: Option<Fetch>,
) -> BoxFuture<Result<OAuthCredentials, OAuthError>> {
    Box::pin(token::refresh(
        refresh_token,
        enterprise_domain,
        fetch.unwrap_or_else(default_fetch),
    ))
}

/// Authorize a device account and finish catalog policy attempts before returning credentials.
pub fn login_github_copilot(
    callbacks: OAuthCallbacks,
    fetch: Option<Fetch>,
) -> BoxFuture<Result<OAuthCredentials, OAuthError>> {
    Box::pin(login_with_clock(
        callbacks,
        fetch.unwrap_or_else(default_fetch),
        crate::records::diagnostics::timestamp_now,
    ))
}

/// Run the owned login with the supplied wall-clock reader.
async fn login_with_clock(
    callbacks: OAuthCallbacks,
    fetch: Fetch,
    clock: impl Fn() -> f64 + Send + Sync,
) -> Result<OAuthCredentials, OAuthError> {
    let input = callbacks
        .on_prompt(OAuthPrompt {
            message: "GitHub Enterprise URL/domain (blank for github.com)".into(),
            placeholder: Some("company.ghe.com".into()),
            allow_empty: Some(true),
        })
        .await?;
    polling::check_cancelled(callbacks.signal())?;
    let enterprise = normalize_domain(&input).filter(|domain| !domain.is_empty());
    if !input.trim_matches(authorization_whitespace).is_empty() && enterprise.is_none() {
        return Err(OAuthError::message("Invalid GitHub Enterprise URL/domain"));
    }
    let domain = enterprise.as_deref().unwrap_or("github.com");
    let mut device = polling::start_device_flow(domain, &fetch).await?;
    callbacks.on_auth(OAuthAuthInfo {
        url: std::mem::take(&mut device.verification_uri),
        instructions: Some(format!("Enter code: {}", device.user_code)),
    })?;
    let access =
        polling::poll_for_github_access_token(domain, &device, &fetch, callbacks.signal(), &clock)
            .await?;
    let credentials = token::refresh(access, enterprise.clone(), fetch.clone()).await?;
    callbacks.on_progress("Enabling models...")?;
    token::enable_all_github_copilot_models(&credentials, enterprise.as_deref(), &fetch).await;
    Ok(credentials)
}

/// Read provider metadata without coercing a malformed operand into a domain.
fn enterprise_domain(credentials: &OAuthCredentials) -> Result<Option<&str>, OAuthError> {
    credentials
        .extra
        .get("enterpriseUrl")
        .map(Option::<&str>::deserialize)
        .transpose()
        .map(Option::flatten)
        .map_err(|error| OAuthError::message(error.to_string()))
}

/// Device-account provider using the shared interaction and transport interfaces.
pub(crate) struct GitHubCopilotOAuthProvider;
impl OAuthProviderInterface for GitHubCopilotOAuthProvider {
    fn id(&self) -> &'static str {
        "github-copilot"
    }
    fn name(&self) -> &'static str {
        "GitHub Copilot"
    }
    fn login(
        &self,
        callbacks: OAuthCallbacks,
        fetch: Option<Fetch>,
    ) -> BoxFuture<Result<OAuthCredentials, OAuthError>> {
        login_github_copilot(callbacks, fetch)
    }
    fn refresh_token(
        &self,
        credentials: OAuthCredentials,
        fetch: Option<Fetch>,
    ) -> BoxFuture<Result<OAuthCredentials, OAuthError>> {
        let domain = enterprise_domain(&credentials).map(|domain| domain.map(str::to_owned));
        Box::pin(async move {
            let mut refreshed = token::refresh(
                credentials.refresh,
                domain?,
                fetch.unwrap_or_else(default_fetch),
            )
            .await?;
            if credentials.extra.get("enterpriseUrl") == Some(&serde_json::Value::Null) {
                refreshed
                    .extra
                    .insert("enterpriseUrl".into(), serde_json::Value::Null);
            }
            Ok(refreshed)
        })
    }
    fn get_api_key<'a>(&self, credentials: &'a OAuthCredentials) -> Result<&'a str, OAuthError> {
        Ok(&credentials.access)
    }
    fn modify_models(
        &self,
        models: Vec<Model>,
        credentials: &OAuthCredentials,
    ) -> Result<Vec<Model>, OAuthError> {
        let domain = match credentials.extra.get("enterpriseUrl") {
            None | Some(serde_json::Value::Null | serde_json::Value::Bool(false)) => None,
            Some(serde_json::Value::Number(number)) if number.as_f64() == Some(0.0) => None,
            Some(serde_json::Value::String(domain)) if domain.is_empty() => None,
            _ => enterprise_domain(credentials)?.and_then(normalize_domain),
        };
        let base = get_github_copilot_base_url(Some(&credentials.access), domain.as_deref());
        Ok(models
            .into_iter()
            .map(|mut model| {
                if model.provider == "github-copilot" {
                    model.base_url.clone_from(&base);
                }
                model
            })
            .collect())
    }
}

/// Device-account provider operations.
#[cfg(not(target_arch = "wasm32"))]
pub const GITHUB_COPILOT_OAUTH_PROVIDER: &(dyn OAuthProviderInterface + Send + Sync) =
    &GitHubCopilotOAuthProvider;
/// Device-account provider operations.
#[cfg(target_arch = "wasm32")]
pub const GITHUB_COPILOT_OAUTH_PROVIDER: &dyn OAuthProviderInterface = &GitHubCopilotOAuthProvider;

#[cfg(test)]
mod tests;
