//! Cloud key and explicit endpoint selection.

/// Trim an authored endpoint, excluding empty and generated template hosts.
pub(super) fn custom_endpoint(text: &str) -> Option<&str> {
    let text = text.trim_matches(maestro_text::is_whitespace_scalar);
    (!text.is_empty() && !text.contains("{location}")).then_some(text)
}

/// Recognize a complete, case-sensitive ASCII API-version pathname segment.
pub(super) fn has_version(text: &str) -> bool {
    let parsed = url::Url::parse(text);
    let path = parsed.as_ref().map_or(text, url::Url::path);
    path.split('/').any(|part| {
        let Some(rest) = part.strip_prefix('v') else {
            return false;
        };
        let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
        digits > 0
            && (rest.len() == digits
                || rest[digits..]
                    .strip_prefix("beta")
                    .is_some_and(|tail| tail.bytes().all(|byte| byte.is_ascii_digit())))
    })
}

/// Select the first nonempty trimmed key before rejecting credential markers.
pub(super) fn select_key<'a>(
    supplied: Option<&'a str>,
    environment: Option<&'a str>,
) -> Option<&'a str> {
    let trim = |text: &'a str| text.trim_matches(maestro_text::is_whitespace_scalar);
    supplied
        .map(trim)
        .filter(|text| !text.is_empty())
        .or_else(|| environment.map(trim))
        .filter(|text| !text.is_empty() && *text != "gcp-vertex-credentials")
        .filter(|text| {
            !text
                .strip_prefix('<')
                .and_then(|text| text.strip_suffix('>'))
                .is_some_and(|inner| !inner.is_empty() && !inner.contains('>'))
        })
}

/// Capture native scope through its separate, non-trimming provider fallback chains.
pub(super) fn scope(
    supplied: Option<String>,
    keys: &[&str],
    message: &str,
) -> Result<String, crate::DiagnosticErrorInfo> {
    supplied
        .filter(|text| !text.is_empty())
        .or_else(|| {
            #[cfg(not(target_arch = "wasm32"))]
            {
                keys.iter().find_map(|key| crate::builtins::env_value(key))
            }
            #[cfg(target_arch = "wasm32")]
            {
                let _ = keys;
                None
            }
        })
        .ok_or_else(|| crate::DiagnosticErrorInfo::error(message))
}

/// Capture explicit cloud settings before asynchronous payload hooks.
pub(super) fn client<E: crate::ProviderExecution>(
    model: &crate::Model,
    options: &super::GoogleVertexOptions<E>,
) -> Result<super::super::google_shared::ApiClient, crate::DiagnosticErrorInfo> {
    use super::super::google_shared::client_options::{Settings, construct};
    #[cfg(not(target_arch = "wasm32"))]
    let environment = crate::builtins::raw_env_value("GOOGLE_CLOUD_API_KEY");
    #[cfg(target_arch = "wasm32")]
    let environment: Option<String> = None;
    let key =
        select_key(options.common.api_key.as_deref(), environment.as_deref()).map(str::to_owned);
    let (project, location) = if key.is_some() {
        (None, None)
    } else {
        (
            Some(scope(
                options.project.clone(),
                &["GOOGLE_CLOUD_PROJECT", "GCLOUD_PROJECT"],
                "Vertex AI requires a project ID. Set GOOGLE_CLOUD_PROJECT/GCLOUD_PROJECT or pass project in options.",
            )?),
            Some(scope(
                options.location.clone(),
                &["GOOGLE_CLOUD_LOCATION"],
                "Vertex AI requires a location. Set GOOGLE_CLOUD_LOCATION or pass location in options.",
            )?),
        )
    };
    let endpoint = custom_endpoint(&model.base_url);
    construct(
        model,
        options.common.headers.as_ref(),
        Settings {
            cloud: true,
            key,
            project,
            location,
            endpoint: endpoint.map(str::to_owned),
            collection: endpoint.is_some(),
            version: if endpoint.is_some_and(has_version) {
                ""
            } else {
                "v1"
            },
        },
    )
}
