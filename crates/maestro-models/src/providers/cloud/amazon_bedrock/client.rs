//! Region, endpoint and authentication choices, and the SDK client that sends the request.

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::fmt;
use std::pin::Pin;
use std::sync::{Arc, Mutex, PoisonError};
use std::task::{Context, Poll};

use aws_config::BehaviorVersion;
use aws_sdk_bedrockruntime::config::{Credentials, Region, StalledStreamProtectionConfig, Token};
use aws_sdk_bedrockruntime::operation::converse_stream::ConverseStreamOutput;
use aws_sdk_bedrockruntime::types::error::ConverseStreamOutputError;
use aws_smithy_runtime_api::client::auth::AuthSchemeId;
use aws_smithy_runtime_api::client::auth::http::HTTP_BEARER_AUTH_SCHEME_ID;
use aws_smithy_runtime_api::client::http::{
    HttpConnector, HttpConnectorFuture, SharedHttpClient, http_client_fn,
};
use aws_smithy_runtime_api::client::orchestrator::{
    HttpRequest as WireRequest, HttpResponse as WireResponse,
};
use aws_smithy_runtime_api::client::result::{ConnectorError, SdkError};
use aws_smithy_runtime_api::http::StatusCode;
use aws_smithy_types::body::SdkBody;
use aws_smithy_types::error::display::DisplayErrorContext;
use aws_smithy_types::error::metadata::ProvideErrorMetadata;
use hyper::body::{Body, Bytes, Frame};
use url::Url;

use super::BedrockOptions;
use super::credentials::default_provider;
use super::proxy::Environment;
use super::request::Prepared;
use crate::providers::transport::{
    Fetch, FetchError, HttpBody, HttpRequest, ProxySelector, RequestFailure, Route,
    configured_fetch,
};
use crate::{Cancellation, Model};

/// How the request is authenticated.
#[derive(Debug, PartialEq)]
pub(super) enum Auth {
    /// The credential chain composed from the native providers.
    Chain,
    /// A bearer token.
    Bearer(String),
    /// Fixed placeholder credentials, for proxies that do not authenticate.
    Placeholder,
}

/// What configuration decides about one request before any SDK call.
pub(super) struct Settings {
    /// Endpoint the SDK must use; absent when the SDK derives it from the region.
    pub(super) endpoint: Option<String>,
    /// Region the SDK must use; absent when the SDK looks it up in the profile.
    pub(super) region: Option<String>,
    /// Profile the SDK must read.
    pub(super) profile: Option<String>,
    /// Authentication.
    pub(super) auth: Auth,
    /// Whether the connection must speak HTTP/1.
    pub(super) http1: bool,
    /// Whether the environment turns proxying on.
    pub(super) proxy: bool,
}

/// The region of a standard runtime endpoint, or none for any other URL.
fn standard_region(endpoint: &str) -> Option<String> {
    let host = Url::parse(endpoint).ok()?.host_str()?.to_lowercase();
    let rest = host.strip_prefix("bedrock-runtime")?;
    let rest = rest.strip_prefix("-fips").unwrap_or(rest);
    let rest = rest.strip_prefix('.')?;
    let rest = rest.strip_suffix(".cn").unwrap_or(rest);
    let region = rest.strip_suffix(".amazonaws.com")?;
    let valid = !region.is_empty()
        && region
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    valid.then(|| region.to_owned())
}

/// The nonempty `region` option, otherwise nonempty `AWS_REGION`, otherwise nonempty
/// `AWS_DEFAULT_REGION`.
pub(super) fn configured_region<'a, E: crate::ProviderExecution>(
    options: &'a BedrockOptions<E>,
    env: &'a Environment,
) -> Option<&'a str> {
    options
        .region
        .as_deref()
        .filter(|region| !region.is_empty())
        .or_else(|| env.get("AWS_REGION"))
        .or_else(|| env.get("AWS_DEFAULT_REGION"))
}

impl Settings {
    /// Decide configuration from the model, the options and the environment.
    ///
    /// # Errors
    /// Fails when the endpoint the SDK must use is not a URL.
    pub(super) fn resolve<E: crate::ProviderExecution>(
        model: &Model,
        options: &BedrockOptions<E>,
        env: &Environment,
    ) -> Result<Self, RequestFailure> {
        let region = configured_region(options, env).map(str::to_owned);
        let profile = options
            .profile
            .as_deref()
            .filter(|profile| !profile.is_empty())
            .or_else(|| env.get("AWS_PROFILE"))
            .map(str::to_owned);
        let endpoint_region = standard_region(&model.base_url);
        let explicit = endpoint_region.is_none() || (region.is_none() && profile.is_none());
        let region = region
            .or_else(|| endpoint_region.filter(|_| explicit))
            .or_else(|| profile.is_none().then(|| "us-east-1".to_owned()));
        let token = options
            .bearer_token
            .as_deref()
            .filter(|token| !token.is_empty())
            .or_else(|| env.get("AWS_BEARER_TOKEN_BEDROCK"));
        let auth = match token {
            _ if env.get("AWS_BEDROCK_SKIP_AUTH") == Some("1") => Auth::Placeholder,
            Some(token) => Auth::Bearer(token.to_owned()),
            None => Auth::Chain,
        };
        let proxy = env.names_proxy();
        let endpoint = explicit
            .then(|| Url::parse(&model.base_url).map(String::from))
            .transpose()
            .map_err(|error| RequestFailure::new(error.to_string()))?;
        Ok(Self {
            endpoint,
            region,
            profile,
            auth,
            http1: proxy || env.get("AWS_BEDROCK_FORCE_HTTP1") == Some("1"),
            proxy,
        })
    }
}

/// Status and request identifier of the latest response the transport returned.
#[derive(Clone, Default)]
pub(super) struct Observed {
    /// HTTP status.
    pub(super) status: u16,
    /// Request identifier header, when it has text.
    pub(super) request_id: Option<String>,
}

/// A response body the SDK can read, moved across threads behind a lock only to be shared.
struct Chunks(Mutex<HttpBody>);

impl Body for Chunks {
    type Data = Bytes;
    type Error = FetchError;

    fn poll_frame(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, FetchError>>> {
        let body = self
            .get_mut()
            .0
            .get_mut()
            .unwrap_or_else(PoisonError::into_inner);
        body.as_mut()
            .poll_next(context)
            .map(|chunk| chunk.map(|chunk| chunk.map(|bytes| Frame::data(Bytes::from(bytes)))))
    }
}

/// The SDK's transport, which sends through a [`Fetch`].
struct Bridge {
    /// Sender of each attempt.
    fetch: Fetch,
    /// Signal handed to the sender.
    signal: Option<Cancellation>,
    /// Latest response seen.
    observed: Arc<Mutex<Option<Observed>>>,
}

impl fmt::Debug for Bridge {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Bridge")
    }
}

impl HttpConnector for Bridge {
    fn call(&self, request: WireRequest) -> HttpConnectorFuture {
        let fetch = Arc::clone(&self.fetch);
        let signal = self.signal.clone();
        let observed = Arc::clone(&self.observed);
        HttpConnectorFuture::new(async move {
            let Some(body) = request.body().bytes() else {
                return Err(ConnectorError::user(
                    "the request body is not in memory".into(),
                ));
            };
            let sent = HttpRequest {
                method: request.method().to_owned(),
                url: request.uri().to_owned(),
                headers: request
                    .headers()
                    .iter()
                    .map(|(name, value)| (name.to_lowercase(), value.to_owned()))
                    .collect(),
                body: body.to_vec(),
                signal,
            };
            let response = fetch(sent).await.map_err(|error| match error {
                FetchError::Timeout => ConnectorError::timeout(error.into()),
                FetchError::Aborted => ConnectorError::user(error.into()),
                FetchError::Connection(_) => ConnectorError::io(error.into()),
            })?;
            let status = StatusCode::try_from(response.status)
                .map_err(|error| ConnectorError::other(error.into(), None))?;
            *observed.lock().unwrap_or_else(PoisonError::into_inner) = Some(Observed {
                status: response.status,
                request_id: ["x-amzn-requestid", "x-amzn-request-id", "x-amz-request-id"]
                    .iter()
                    .find_map(|name| response.headers.get(*name))
                    .filter(|id| !id.is_empty())
                    .cloned(),
            });
            let mut wire = WireResponse::new(
                status,
                SdkBody::from_body_1_x(Chunks(Mutex::new(response.body))),
            );
            for (name, value) in &response.headers {
                wire.headers_mut()
                    .try_insert(name.clone(), value.clone())
                    .ok();
            }
            Ok(wire)
        })
    }
}

/// An SDK client ready to send, with the response its transport last saw.
pub(super) struct Service {
    /// The client.
    client: aws_sdk_bedrockruntime::Client,
    /// Latest response the transport returned.
    observed: Arc<Mutex<Option<Observed>>>,
}

/// Default client routing shaped by the settings.
fn transport(settings: &Settings, env: Environment) -> Route {
    let env = Arc::new(env);
    let proxy: Option<ProxySelector> = settings
        .proxy
        .then(|| Arc::new(move |url: &Url| env.proxy_for(url)) as ProxySelector);
    Route {
        proxy,
        http1_only: settings.http1,
        ..Route::default()
    }
}

/// Adapt a sender to the native SDK through the single connector implementation.
fn bridge(
    fetch: Option<Fetch>,
    route: Route,
    signal: Option<Cancellation>,
    observed: Arc<Mutex<Option<Observed>>>,
) -> SharedHttpClient {
    let senders = Mutex::new(BTreeMap::new());
    http_client_fn(move |settings, _| {
        let fetch = fetch.clone().unwrap_or_else(|| {
            let key = (settings.connect_timeout(), settings.read_timeout());
            senders
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .entry(key)
                .or_insert_with(|| {
                    configured_fetch(Route {
                        connect_timeout: key.0,
                        read_timeout: key.1,
                        ..route.clone()
                    })
                })
                .clone()
        });
        aws_smithy_runtime_api::client::http::SharedHttpConnector::new(Bridge {
            fetch,
            signal: signal.clone(),
            observed: Arc::clone(&observed),
        })
    })
}

/// Build the SDK client from the settings. The region is looked up while the shared
/// configuration loads; credentials are looked up by the SDK on the first send.
pub(super) async fn connect<E: crate::ProviderExecution>(
    settings: &Settings,
    options: &BedrockOptions<E>,
    env: Environment,
) -> Service {
    let observed = Arc::new(Mutex::new(None));
    let empty_profile = env.0.get("AWS_PROFILE").is_some_and(String::is_empty);
    let http = bridge(
        options.common.fetch.clone(),
        transport(settings, env),
        options.common.signal.clone(),
        Arc::clone(&observed),
    );
    let mut loader = aws_config::defaults(BehaviorVersion::latest());
    if let Some(profile) = &settings.profile {
        loader = loader.profile_name(profile);
    } else if empty_profile {
        loader = loader.profile_name("default");
    }
    if let Some(region) = &settings.region {
        loader = loader.region(Region::new(region.clone()));
    }
    let shared = loader.load().await.into_builder().http_client(http).build();
    let mut config = aws_sdk_bedrockruntime::config::Builder::from(&shared)
        .stalled_stream_protection(StalledStreamProtectionConfig::disabled());
    if let Some(endpoint) = &settings.endpoint {
        config = config.endpoint_url(endpoint);
    }
    config = match &settings.auth {
        Auth::Chain => {
            let direct = bridge(None, Route::default(), None, Arc::new(Mutex::new(None)));
            config.credentials_provider(default_provider(
                settings.profile.as_deref(),
                &shared,
                &direct,
            ))
        }
        Auth::Bearer(token) => config
            .token_provider(Token::new(token, None))
            .auth_scheme_preference([HTTP_BEARER_AUTH_SCHEME_ID]),
        Auth::Placeholder => config
            .credentials_provider(Credentials::new(
                "dummy-access-key",
                "dummy-secret-key",
                None,
                None,
                "placeholder",
            ))
            .auth_scheme_preference([AuthSchemeId::from("sigv4")]),
    };
    Service {
        client: aws_sdk_bedrockruntime::Client::from_conf(config.build()),
        observed,
    }
}

impl Service {
    /// Send the prepared request: the SDK binds the route and signs, and the body is replaced
    /// by the encoded one just before signing.
    ///
    /// # Errors
    /// Fails with the SDK's service error under its readable prefix, otherwise with the SDK's
    /// own error text.
    pub(super) async fn send(
        &self,
        prepared: Prepared,
    ) -> Result<ConverseStreamOutput, RequestFailure> {
        let Prepared { model_id, body } = prepared;
        self.client
            .converse_stream()
            .set_model_id(model_id)
            .customize()
            .mutate_request(move |request| {
                *request.body_mut() = SdkBody::from(body.clone());
                request
                    .headers_mut()
                    .insert("content-length", body.len().to_string());
            })
            .send()
            .await
            .map_err(|error| describe(&error, |_| None))
    }

    /// The latest response the transport returned, once.
    pub(super) fn observed(&self) -> Option<Observed> {
        self.observed
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
    }
}

/// Response headers the observation hook sees: the request identifier when there is one.
pub(super) fn response_headers(observed: &Observed) -> BTreeMap<String, String> {
    observed
        .request_id
        .iter()
        .map(|id| ("x-amzn-requestid".to_owned(), id.clone()))
        .collect()
}

/// Describe an SDK failure: a service error reads `<prefix>: <message>` and anything else
/// reads as the SDK's error with its causes. `class` names the error class when the error does
/// not carry its own code.
pub(super) fn describe<E, R>(
    error: &SdkError<E, R>,
    class: fn(&E) -> Option<&'static str>,
) -> RequestFailure
where
    E: ProvideErrorMetadata + std::error::Error + Send + Sync + 'static,
    R: fmt::Debug,
{
    let Some(service) = error.as_service_error() else {
        return RequestFailure::new(DisplayErrorContext(error).to_string());
    };
    let name = class(service)
        .or_else(|| service.code())
        .unwrap_or("UnknownError");
    let prefix = match name {
        "InternalServerException" => "Internal server error",
        "ModelStreamErrorException" => "Model stream error",
        "ValidationException" => "Validation error",
        "ThrottlingException" => "Throttling error",
        "ServiceUnavailableException" => "Service unavailable",
        other => other,
    };
    RequestFailure::new(format!(
        "{prefix}: {}",
        service.message().unwrap_or_default()
    ))
}

/// The class a streamed service error belongs to.
pub(super) fn streamed_class(error: &ConverseStreamOutputError) -> Option<&'static str> {
    match error {
        ConverseStreamOutputError::InternalServerException(_) => Some("InternalServerException"),
        ConverseStreamOutputError::ModelStreamErrorException(_) => {
            Some("ModelStreamErrorException")
        }
        ConverseStreamOutputError::ValidationException(_) => Some("ValidationException"),
        ConverseStreamOutputError::ThrottlingException(_) => Some("ThrottlingException"),
        ConverseStreamOutputError::ServiceUnavailableException(_) => {
            Some("ServiceUnavailableException")
        }
        _ => None,
    }
}
