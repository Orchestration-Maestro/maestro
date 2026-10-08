//! Cloudflare account endpoints resolved from the environment.

use std::sync::LazyLock;

use regex::Regex;

use crate::{DiagnosticErrorInfo, Model};

/// Workers AI direct endpoint.
pub const CLOUDFLARE_WORKERS_AI_BASE_URL: &str =
    "https://api.cloudflare.com/client/v4/accounts/{CLOUDFLARE_ACCOUNT_ID}/ai/v1";

/// AI Gateway unified endpoint.
pub const CLOUDFLARE_AI_GATEWAY_COMPAT_BASE_URL: &str =
    "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/compat";

/// AI Gateway passthrough to the response protocol.
pub const CLOUDFLARE_AI_GATEWAY_OPENAI_BASE_URL: &str =
    "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/openai";

/// AI Gateway passthrough to the message protocol.
pub const CLOUDFLARE_AI_GATEWAY_ANTHROPIC_BASE_URL: &str = "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/anthropic";

/// Matches `{NAME}` placeholders written with uppercase identifier characters.
static PLACEHOLDER: LazyLock<Result<Regex, regex::Error>> =
    LazyLock::new(|| Regex::new(r"\{([A-Z_][A-Z0-9_]*)\}"));

/// Report whether a provider identifier names a Cloudflare product.
#[must_use]
pub fn is_cloudflare_provider(provider: &str) -> bool {
    matches!(provider, "cloudflare-workers-ai" | "cloudflare-ai-gateway")
}

/// Substitute each `{NAME}` placeholder in the model's base URL from the environment.
///
/// # Errors
/// Names the first variable that is unset or empty.
pub fn resolve_cloudflare_base_url(model: &Model) -> Result<String, DiagnosticErrorInfo> {
    let url = &model.base_url;
    let Ok(placeholder) = PLACEHOLDER.as_ref() else {
        return Ok(url.clone());
    };
    let mut resolved = String::with_capacity(url.len());
    let mut copied = 0;
    for captures in placeholder.captures_iter(url) {
        let (whole, name) = (&captures[0], &captures[1]);
        let value = std::env::var(name)
            .ok()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| missing_variable(name, &model.provider))?;
        let start = captures.get(0).map_or(copied, |found| found.start());
        resolved.push_str(&url[copied..start]);
        resolved.push_str(&value);
        copied = start + whole.len();
    }
    resolved.push_str(&url[copied..]);
    Ok(resolved)
}

/// Build the failure for an unset placeholder variable.
fn missing_variable(name: &str, provider: &str) -> DiagnosticErrorInfo {
    DiagnosticErrorInfo {
        name: Some("Error".into()),
        message: format!("{name} is required for provider {provider} but is not set."),
        stack: None,
        code: None,
    }
}
