//! Metadata-server admission and endpoint identity.

pub(super) mod gcp_residency;

use super::base_external_client::{BaseExternalAccountClient, Effects};
use super::compute_client::Compute;
use super::google_auth::configuration_error;
use super::oauth_client_auth_handler;
use crate::DiagnosticErrorInfo;
use crate::builtins::env_value;
use crate::providers::http::{RequestPolicy, RetryStatuses};
use crate::{Fetch, HttpRequest};
use google_cloud_auth::credentials::Credentials;
use std::time::Duration;

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

/// The documented metadata detection control when metadata is not disabled.
#[derive(Clone, Copy)]
pub(super) enum Detection {
    /// Residency admits; otherwise probe.
    PingOnly,
    /// Metadata is admitted without a probe.
    AssumePresent,
    /// Residency alone decides admission.
    BiosOnly,
}

impl Detection {
    /// Read the control once: `None` means metadata is disabled.
    ///
    /// # Errors
    /// Fails for an unrecognized control value.
    pub(super) fn parse(policy: Option<&str>) -> Result<Option<Self>, DiagnosticErrorInfo> {
        let normalized = policy
            .unwrap_or_default()
            .trim_matches(maestro_text::is_whitespace_scalar)
            .to_ascii_lowercase();
        match normalized.as_str() {
            "none" => Ok(None),
            "assume-present" => Ok(Some(Self::AssumePresent)),
            "bios-only" => Ok(Some(Self::BiosOnly)),
            "" | "ping-only" => Ok(Some(Self::PingOnly)),
            _ => Err(configuration_error()),
        }
    }
}

/// Whether the metadata server is usable, decided before any probe.
#[derive(Clone, Copy)]
enum Admission {
    /// Metadata is available without a probe.
    Present,
    /// Metadata is not available and no probe is allowed.
    Unavailable,
    /// A probe decides availability.
    Probe,
}

/// One resolved endpoint and admission policy, reused by the token source.
pub(super) struct Metadata {
    /// Primary endpoint; a secondary winner never replaces it.
    pub(super) base: String,
    /// Default hostname used only for unoverridden discovery.
    secondary: Option<String>,
    /// Complete attempt deadline, absent on resident hosts.
    pub(super) timeout: Option<Duration>,
    /// Probe no-response budget, independent of token retries.
    pub(super) retries: u64,
    /// Admission decided from the control and residency.
    admission: Admission,
}

impl Metadata {
    /// Capture the already-observed environment and residency without probing.
    pub(super) fn new(
        detection: Detection,
        resident: bool,
        endpoints: (Option<&str>, Option<&str>),
        retries: u64,
    ) -> Result<Self, DiagnosticErrorInfo> {
        let admission = match detection {
            Detection::AssumePresent => Admission::Present,
            _ if resident => Admission::Present,
            Detection::BiosOnly => Admission::Unavailable,
            Detection::PingOnly => Admission::Probe,
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
            admission,
        })
    }

    /// Apply admission before any token exchange.
    pub(super) async fn admitted(&self, fetch: Fetch) -> bool {
        match self.admission {
            Admission::Present => return true,
            Admission::Unavailable => return false,
            Admission::Probe => {}
        }
        let primary = Box::pin(self.probe(&self.base, fetch.clone()));
        let Some(secondary) = &self.secondary else {
            return primary.await.is_ok();
        };
        let secondary = Box::pin(self.probe(secondary, fetch));
        match futures_util::future::select(primary, secondary).await {
            futures_util::future::Either::Left((result, other))
            | futures_util::future::Either::Right((result, other)) => {
                result.is_ok() || other.await.is_ok()
            }
        }
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
///
/// The availability result is stored in `available`, a negative one included.
pub(super) async fn credentials(
    effects: Effects,
    available: &tokio::sync::OnceCell<bool>,
) -> Result<Credentials, DiagnosticErrorInfo> {
    let Some(detection) = Detection::parse(env_value("METADATA_SERVER_DETECTION").as_deref())?
    else {
        return Err(DiagnosticErrorInfo::error(super::NO_CREDENTIALS));
    };
    let ip = env_value("GCE_METADATA_IP");
    let host = env_value("GCE_METADATA_HOST");
    let retries = env_value("DETECT_GCP_RETRIES")
        .and_then(|value| {
            value
                .trim_matches(maestro_text::is_whitespace_scalar)
                .parse()
                .ok()
        })
        .unwrap_or(0);
    let metadata = Metadata::new(
        detection,
        gcp_residency::detect_gcp_residency(),
        (ip.as_deref(), host.as_deref()),
        retries,
    )?;
    if !*available
        .get_or_init(|| metadata.admitted(effects.fetch.clone()))
        .await
    {
        return Err(DiagnosticErrorInfo::error(super::NO_CREDENTIALS));
    }
    Ok(Credentials::from(BaseExternalAccountClient::new(
        Compute::new(metadata),
        effects,
        env_value("GOOGLE_CLOUD_QUOTA_PROJECT"),
        None,
    )))
}
