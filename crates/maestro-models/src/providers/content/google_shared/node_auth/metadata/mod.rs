//! Metadata-server admission and endpoint identity.

pub(in crate::providers::content::google_shared::node_auth) mod gcp_residency;

use super::google_auth::configuration_error;
use crate::DiagnosticErrorInfo;

/// Resolve endpoint precedence once, retaining schemes and replacing the base path.
pub(super) fn base_url(
    ip: Option<&str>,
    host: Option<&str>,
) -> Result<String, DiagnosticErrorInfo> {
    let selected = ip
        .filter(|value| !value.is_empty())
        .or_else(|| host.filter(|value| !value.is_empty()))
        .unwrap_or("169.254.169.254");
    let has_scheme = selected
        .get(..7)
        .is_some_and(|text| text.eq_ignore_ascii_case("http://"))
        || selected
            .get(..8)
            .is_some_and(|text| text.eq_ignore_ascii_case("https://"));
    let text = if has_scheme {
        selected.to_owned()
    } else {
        format!("http://{selected}")
    };
    let url = url::Url::parse(&text)
        .and_then(|url| url.join("/computeMetadata/v1"))
        .map_err(|_| configuration_error())?;
    Ok(url.into())
}

use super::oauth_client_auth_handler;
use crate::providers::http::{RequestPolicy, RetryStatuses};
use crate::{Fetch, HttpRequest};
use std::time::Duration;

/// Admission selected from the documented metadata detection control.
#[derive(Clone, Copy)]
enum Detection {
    /// Residency admission, otherwise probe.
    Probe,
    /// No metadata operation is allowed.
    None,
    /// Metadata is admitted without a probe.
    Present,
    /// Residency alone decides admission.
    Bios(bool),
}

/// One resolved endpoint and admission policy, reused by the token source.
pub(in crate::providers::content::google_shared::node_auth) struct Metadata {
    /// Primary endpoint; a secondary winner never replaces it.
    pub(super) base: String,
    /// Default hostname used only for unoverridden discovery.
    secondary: Option<String>,
    /// Complete attempt deadline, absent on resident hosts.
    pub(super) timeout: Option<Duration>,
    /// Probe no-response budget, independent of token retries.
    pub(super) retries: u64,
    /// Selected admission control.
    detection: Detection,
}

impl Metadata {
    /// Capture the already-observed environment and residency without probing.
    pub(in crate::providers::content::google_shared::node_auth) fn new(
        policy: Option<&str>,
        resident: bool,
        endpoints: (Option<&str>, Option<&str>),
        retries: u64,
    ) -> Result<Self, DiagnosticErrorInfo> {
        let normalized = policy
            .unwrap_or_default()
            .trim_matches(maestro_text::is_whitespace_scalar)
            .to_ascii_lowercase();
        let detection = match normalized.as_str() {
            "none" => Detection::None,
            _ if resident => Detection::Present,
            "assume-present" => Detection::Present,
            "bios-only" => Detection::Bios(false),
            "" | "ping-only" => Detection::Probe,
            _ => return Err(configuration_error()),
        };
        let overridden = [endpoints.0, endpoints.1]
            .into_iter()
            .flatten()
            .any(|value| !value.is_empty());
        Ok(Self {
            base: base_url(endpoints.0, endpoints.1)?,
            secondary: (!overridden)
                .then(|| "http://metadata.google.internal./computeMetadata/v1".into()),
            timeout: (!resident).then_some(Duration::from_millis(3000)),
            retries,
            detection,
        })
    }

    /// Apply admission before any token exchange.
    pub(in crate::providers::content::google_shared::node_auth) async fn admitted(
        &self,
        fetch: Fetch,
    ) -> Result<bool, DiagnosticErrorInfo> {
        match self.detection {
            Detection::None => return Ok(false),
            Detection::Present => return Ok(true),
            Detection::Bios(resident) => return Ok(resident),
            Detection::Probe => {}
        }
        let primary = Box::pin(self.probe(&self.base, fetch.clone()));
        let Some(secondary) = &self.secondary else {
            return Ok(primary.await.is_ok());
        };
        let secondary = Box::pin(self.probe(secondary, fetch));
        Ok(
            match futures_util::future::select(primary, secondary).await {
                futures_util::future::Either::Left((result, other))
                | futures_util::future::Either::Right((result, other)) => {
                    result.is_ok() || other.await.is_ok()
                }
            },
        )
    }

    /// Probe one candidate over the shared buffered sender and selected retry/deadline policy.
    async fn probe(
        &self,
        base: &str,
        fetch: Fetch,
    ) -> Result<(), google_cloud_auth::errors::CredentialsError> {
        let request = HttpRequest {
            method: "GET".into(),
            url: format!("{base}/instance"),
            headers: [("metadata-flavor".into(), "Google".into())].into(),
            body: Vec::new(),
            signal: None,
        };
        let policy = RequestPolicy {
            timeout: self.timeout,
            retries: 3,
            no_response_retries: self.retries,
            statuses: RetryStatuses::Authentication,
        };
        let response = oauth_client_auth_handler::request(request, fetch, policy).await?;
        validate_flavor(&response)
    }
}

/// Metadata responses must explicitly identify the expected service.
pub(super) fn validate_flavor(
    response: &crate::HttpResponse,
) -> Result<(), google_cloud_auth::errors::CredentialsError> {
    if response
        .headers
        .get("metadata-flavor")
        .is_some_and(|value| value == "Google")
    {
        Ok(())
    } else {
        Err(super::base_external_client::invalid_token())
    }
}

/// Resolve and admit the one metadata source after file discovery is exhausted.
pub(super) async fn credentials(
    effects: super::base_external_client::Effects,
) -> Result<google_cloud_auth::credentials::Credentials, DiagnosticErrorInfo> {
    use super::google_auth::environment;
    let policy = environment("METADATA_SERVER_DETECTION");
    let normalized = policy
        .as_deref()
        .unwrap_or_default()
        .trim_matches(maestro_text::is_whitespace_scalar);
    if normalized.eq_ignore_ascii_case("none") {
        return Err(DiagnosticErrorInfo::error(super::NO_CREDENTIALS));
    }
    let resident = gcp_residency::detect();
    let ip = environment("GCE_METADATA_IP");
    let host = environment("GCE_METADATA_HOST");
    let retries = environment("DETECT_GCP_RETRIES")
        .and_then(|value| {
            value
                .trim_matches(maestro_text::is_whitespace_scalar)
                .parse()
                .ok()
        })
        .unwrap_or(0);
    let metadata = Metadata::new(
        policy.as_deref(),
        resident,
        (ip.as_deref(), host.as_deref()),
        retries,
    )?;
    if !metadata.admitted(effects.fetch.clone()).await? {
        return Err(DiagnosticErrorInfo::error(super::NO_CREDENTIALS));
    }
    let source = super::compute_client::Compute::new(metadata);
    Ok(google_cloud_auth::credentials::Credentials::from(
        super::base_external_client::BaseExternalClient::new(
            source,
            effects,
            &serde_json::json!({"quota_project_id":environment("GOOGLE_CLOUD_QUOTA_PROJECT")}),
        ),
    ))
}
