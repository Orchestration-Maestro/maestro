//! Constructor-time endpoint, authentication and resource selection.

use super::api_client::{ApiClient, Authentication, Resource};
use crate::{DiagnosticErrorInfo, Model};
use indexmap::IndexMap;

/// Trim the SDK's environment observations, preserving present-empty flags.
fn environment(key: &str) -> Option<String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        crate::builtins::raw_env_value(key).map(|text| {
            text.trim_matches(maestro_text::is_whitespace_scalar)
                .to_owned()
        })
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = key;
        None
    }
}

/// Capture public constructor inputs, including ambient cloud selection.
pub(in crate::providers::content) fn public(
    model: &Model,
    key: &str,
    headers: Option<&IndexMap<String, String>>,
) -> Result<ApiClient, DiagnosticErrorInfo> {
    let cloud = environment("GOOGLE_GENAI_USE_ENTERPRISE")
        .or_else(|| environment("GOOGLE_GENAI_USE_VERTEXAI"))
        .is_some_and(|text| text.eq_ignore_ascii_case("true"));
    let mut project = environment("GOOGLE_CLOUD_PROJECT");
    let mut location = environment("GOOGLE_CLOUD_LOCATION");
    let implicit_key = environment("GOOGLE_API_KEY")
        .filter(|key| !key.is_empty())
        .or_else(|| environment("GEMINI_API_KEY").filter(|key| !key.is_empty()));
    let has_scope = project.as_ref().is_some_and(|text| !text.is_empty())
        || location.as_ref().is_some_and(|text| !text.is_empty());
    let selected_key = if cloud && has_scope && !key.is_empty() {
        project = None;
        location = None;
        Some(key.to_owned())
    } else if cloud && has_scope && implicit_key.is_some() {
        None
    } else {
        Some(key.to_owned())
    };
    let explicit = (!model.base_url.is_empty()).then(|| model.base_url.clone());
    let explicit_version = explicit.is_some();
    construct(
        model,
        headers,
        Settings {
            cloud,
            key: selected_key,
            project,
            location,
            endpoint: explicit,
            collection: false,
            version: if explicit_version {
                ""
            } else if cloud {
                "v1beta1"
            } else {
                "v1beta"
            },
        },
    )
}

/// Fully selected inputs before the SDK client normalizes resource identity.
pub(in crate::providers::content) struct Settings {
    /// Cloud rather than public projection.
    pub cloud: bool,
    /// Authentication key, retaining present-empty separately from native auth.
    pub key: Option<String>,
    /// Constructor project.
    pub project: Option<String>,
    /// Constructor location.
    pub location: Option<String>,
    /// Explicit endpoint before environment fallback.
    pub endpoint: Option<String>,
    /// Explicit cloud endpoint selects collection resource scope.
    pub collection: bool,
    /// Constructor API version.
    pub version: &'static str,
}

/// Capture one client while leaving native credential discovery lazy.
pub(in crate::providers::content) fn construct(
    model: &Model,
    headers: Option<&IndexMap<String, String>>,
    mut settings: Settings,
) -> Result<ApiClient, DiagnosticErrorInfo> {
    let has_key = settings.key.as_ref().is_some_and(|text| !text.is_empty());
    let routing_key = settings.normalize_resource(has_key);
    #[cfg(not(target_arch = "wasm32"))]
    let authentication = settings.key.take().map_or_else(
        || Authentication::Native(super::node_auth::NodeAuth::default()),
        Authentication::ApiKey,
    );
    #[cfg(target_arch = "wasm32")]
    let authentication = Authentication::ApiKey(
        settings
            .key
            .take()
            .filter(|key| !key.is_empty())
            .ok_or_else(|| {
                DiagnosticErrorInfo::error("An API Key must be set when running in a browser")
            })?,
    );
    let endpoint = settings.endpoint.take().or_else(|| {
        environment(if settings.cloud {
            "GOOGLE_VERTEX_BASE_URL"
        } else {
            "GOOGLE_GEMINI_BASE_URL"
        })
        .filter(|text| !text.is_empty())
    });
    validate(&settings, endpoint.as_deref(), routing_key)?;
    if settings.cloud && endpoint.is_some() && !has_key && !settings.has_scope() {
        settings.project = None;
        settings.location = None;
    }
    let mut client = super::super::google::create_client(model, "", headers, None);
    client.base_url = endpoint.unwrap_or_else(|| default_endpoint(&settings, routing_key));
    client.api_version = settings.version.into();
    client.authentication = authentication;
    client.resource = Resource {
        cloud: settings.cloud,
        project: settings.project,
        location: settings.location,
        has_key: routing_key,
        collection: settings.collection,
    };
    Ok(client)
}

/// Validate constructor authentication before any payload callback.
fn validate(
    settings: &Settings,
    endpoint: Option<&str>,
    routing_key: bool,
) -> Result<(), DiagnosticErrorInfo> {
    if settings.cloud && !settings.has_scope() && !routing_key && endpoint.is_none() {
        return Err(DiagnosticErrorInfo::error(
            "Authentication is not set up. Please provide either a project and location, or an API key, or a custom base URL.",
        ));
    }
    Ok(())
}

/// Default cloud hosts distinguish global, multi-region and regional locations.
fn default_endpoint(settings: &Settings, routing_key: bool) -> String {
    if !settings.cloud {
        return "https://generativelanguage.googleapis.com".into();
    }
    let location = settings.location.as_deref().unwrap_or("global");
    if routing_key || location == "global" {
        "https://aiplatform.googleapis.com".into()
    } else if matches!(location, "us" | "eu") {
        format!("https://aiplatform.{location}.rep.googleapis.com")
    } else {
        format!("https://{location}-aiplatform.googleapis.com")
    }
}

impl Settings {
    /// Whether the constructor has both nonempty scope values.
    fn has_scope(&self) -> bool {
        self.project.as_ref().is_some_and(|text| !text.is_empty())
            && self.location.as_ref().is_some_and(|text| !text.is_empty())
    }

    /// Normalize only resource identity, keeping authentication captured independently.
    fn normalize_resource(&mut self, has_key: bool) -> bool {
        let routing_key = has_key && !(self.cloud && self.has_scope());
        if self.cloud {
            if !self.has_scope() && has_key {
                self.project = None;
                self.location = None;
            }
            if !has_key && self.location.as_ref().is_none_or(String::is_empty) {
                self.location = Some("global".into());
            }
        }
        routing_key
    }
}
