//! Local-server model discovery using supplied transport and diagnostic effects.
use futures_util::StreamExt;
use maestro_models::{Fetch, FetchError, HttpRequest, Model, ModelCost, ModelInput};
use serde::de::DeserializeOwned;
use serde_json::value::RawValue;
use std::{collections::HashMap, fmt, sync::Arc};

/// Local model listing protocol.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiscoveryProvider {
    /// Ollama tags and show operations.
    Ollama,
    /// llama.cpp model listing.
    LlamaCpp,
    /// vLLM model listing.
    Vllm,
    /// LM Studio system socket RPC.
    Lmstudio,
}
impl fmt::Display for DiscoveryProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Ollama => "Ollama",
            Self::LlamaCpp => "llama.cpp",
            Self::Vllm => "vLLM",
            Self::Lmstudio => "LM Studio",
        })
    }
}
/// Socket operation that failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SocketPhase {
    /// Opening the connection.
    Connection,
    /// Sending or receiving authentication.
    Authentication,
    /// Sending or receiving the listing call.
    Rpc,
}
/// Why discovery could not produce a model listing.
#[derive(Debug)]
pub enum DiscoveryErrorKind {
    /// Retained HTTP transport failure.
    Transport(Box<FetchError>),
    /// Refused HTTP status.
    Http(u16),
    /// Unusable JSON response or selected model fields.
    InvalidResponse,
    /// Invalid server host.
    InvalidUrl,
    /// Original socket I/O failure and its operation phase.
    Socket {
        /// Operation phase.
        phase: SocketPhase,
        /// Retained native cause.
        cause: Box<std::io::Error>,
    },
    /// Authentication refusal or malformed authentication response.
    Authentication(Box<std::io::Error>),
    /// Complete remote RPC error object.
    Rpc(Box<RawValue>),
}
/// Provider-attributed discovery failure.
#[derive(Debug)]
pub struct DiscoveryError {
    /// Operation that failed.
    pub provider: DiscoveryProvider,
    /// Failure category with retained transport cause.
    pub kind: DiscoveryErrorKind,
    /// Failure detail.
    pub message: String,
}
impl fmt::Display for DiscoveryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.provider == DiscoveryProvider::Lmstudio {
            return write!(f, "LM Studio discovery failed: {}", self.message);
        }
        write!(
            f,
            "Failed to discover {} models: {}",
            self.provider, self.message
        )
    }
}
impl std::error::Error for DiscoveryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            DiscoveryErrorKind::Transport(error) => Some(error.as_ref()),
            DiscoveryErrorKind::Socket { cause, .. }
            | DiscoveryErrorKind::Authentication(cause) => Some(cause.as_ref()),
            _ => None,
        }
    }
}
/// A diagnostic delivered synchronously at the operation's failure boundary.
pub enum DiscoveryDiagnostic<'a> {
    /// Detail response does not advertise tool use.
    SkippedWithoutTools {
        /// Listed model name.
        model: &'a str,
    },
    /// A single model detail failed.
    DetailFailed {
        /// Listed model name.
        model: &'a str,
        /// Retained detail failure.
        error: &'a DiscoveryError,
    },
    /// The whole discovery operation failed.
    Failed(&'a DiscoveryError),
}
impl fmt::Display for DiscoveryDiagnostic<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SkippedWithoutTools { model } => {
                write!(f, "Skipping model {model}: does not support tools")
            }
            Self::DetailFailed { model, error } => write!(
                f,
                "Failed to fetch details for model {model}: {}",
                error.message
            ),
            Self::Failed(error) => error.fmt(f),
        }
    }
}
/// Synchronous native diagnostic destination.
#[cfg(not(target_arch = "wasm32"))]
pub type DiscoveryLog = Arc<dyn for<'a> Fn(DiscoveryDiagnostic<'a>) + Send + Sync>;
/// Synchronous browser-local diagnostic destination.
#[cfg(target_arch = "wasm32")]
pub type DiscoveryLog = Arc<dyn for<'a> Fn(DiscoveryDiagnostic<'a>)>;
/// Caller-supplied discovery effects.
pub struct DiscoveryOptions {
    /// HTTP operation without additional retry policy.
    pub fetch: Fetch,
    /// Diagnostic destination.
    pub log: DiscoveryLog,
    /// Complete-message socket factory without retry policy.
    pub connect_web_socket: maestro_models::ConnectWebSocket,
}
/// Last-wins JSON members, decoded only when selected.
type Members = HashMap<String, Box<RawValue>>;
/// Decode an object without recursively interpreting unread members.
fn members(raw: &str) -> Result<Members, serde_json::Error> {
    maestro_json::object::from_str(raw)
}
/// Read a selected required member.
fn field<T: DeserializeOwned>(object: &Members, key: &str) -> Result<T, serde_json::Error> {
    serde_json::from_str(object.get(key).map_or("null", |value| value.get()))
}
/// Read a finite number after the response's truthy default.
fn limit(object: &Members, key: &str, fallback: f64) -> Result<f64, serde_json::Error> {
    match object
        .get(key)
        .filter(|value| maestro_json::is_raw_truthy(value))
    {
        Some(value) => serde_json::from_str::<f64>(value.get()),
        None => Ok(fallback),
    }
}
/// Attribute invalid selected response data.
fn invalid(provider: DiscoveryProvider, error: impl fmt::Display) -> DiscoveryError {
    DiscoveryError {
        provider,
        kind: DiscoveryErrorKind::InvalidResponse,
        message: error.to_string(),
    }
}
/// Retain the original transport cause.
fn transport(provider: DiscoveryProvider, error: FetchError) -> DiscoveryError {
    DiscoveryError {
        provider,
        message: error.to_string(),
        kind: DiscoveryErrorKind::Transport(Box::new(error)),
    }
}
/// Request and decode bytes, checking status before JSON admission.
async fn http(
    provider: DiscoveryProvider,
    request: HttpRequest,
    options: &DiscoveryOptions,
) -> Result<Members, DiscoveryError> {
    let mut response = (options.fetch)(request)
        .await
        .map_err(|error| transport(provider, error))?;
    if !(200..300).contains(&response.status) {
        return Err(DiscoveryError {
            provider,
            kind: DiscoveryErrorKind::Http(response.status),
            message: format!("HTTP {}: {}", response.status, response.status_text),
        });
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.body.next().await {
        bytes.extend(chunk.map_err(|error| transport(provider, error))?);
    }
    members(&maestro_text::decode_utf8(&bytes)).map_err(|error| invalid(provider, error))
}
/// Final invocation identity and display name.
struct ModelNames {
    /// Downloaded path or listed identifier.
    id: String,
    /// Final display name.
    name: String,
}
impl ModelNames {
    /// Use the same listed name for invocation and display.
    fn same(id: String) -> Self {
        Self {
            name: id.clone(),
            id,
        }
    }
}
/// Construct a text-only, unpriced completion model.
fn model(names: ModelNames, base: &str, context: f64, maximum: f64, reasoning: bool) -> Model {
    Model {
        name: names.name,
        id: names.id,
        api: "openai-completions".into(),
        provider: String::new(),
        base_url: format!("{base}/v1"),
        reasoning,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: context,
        max_tokens: maximum,
        headers: None,
        compat: None,
    }
}
/// Read a llama.cpp listing with optional bearer authentication.
///
/// # Errors
/// Returns provider-attributed transport, HTTP or invalid-response failures.
pub async fn discover_llama_cpp_models(
    base_url: &str,
    api_key: Option<&str>,
    options: &DiscoveryOptions,
) -> Result<Vec<Model>, DiscoveryError> {
    let provider = DiscoveryProvider::LlamaCpp;
    let result = completion_models(provider, base_url, api_key, options).await;
    if let Err(error) = &result {
        (options.log)(DiscoveryDiagnostic::Failed(error));
    }
    result
}
/// Read the selected completion server's model list.
async fn completion_models(
    provider: DiscoveryProvider,
    base: &str,
    key: Option<&str>,
    options: &DiscoveryOptions,
) -> Result<Vec<Model>, DiscoveryError> {
    let mut request = HttpRequest {
        method: "GET".into(),
        url: format!("{base}/v1/models"),
        headers: [("content-type".into(), "application/json".into())]
            .into_iter()
            .collect(),
        body: Vec::new(),
        signal: None,
    };
    if let Some(key) = key.filter(|key| !key.is_empty()) {
        request
            .headers
            .insert("authorization".into(), format!("Bearer {key}"));
    }
    let object = http(provider, request, options).await?;
    let rows: Vec<Box<RawValue>> =
        field(&object, "data").map_err(|error| invalid(provider, error))?;
    rows.into_iter()
        .map(|row| {
            let object = members(row.get()).map_err(|error| invalid(provider, error))?;
            let id = field(&object, "id").map_err(|error| invalid(provider, error))?;
            let context_key = if provider == DiscoveryProvider::Vllm {
                "max_model_len"
            } else {
                "context_length"
            };
            let context =
                limit(&object, context_key, 8192.0).map_err(|error| invalid(provider, error))?;
            let maximum = if provider == DiscoveryProvider::Vllm {
                context.min(4096.0)
            } else {
                limit(&object, "max_tokens", 4096.0).map_err(|error| invalid(provider, error))?
            };
            Ok(model(ModelNames::same(id), base, context, maximum, false))
        })
        .collect()
}

/// Read a vLLM listing with optional bearer authentication.
///
/// # Errors
/// Returns provider-attributed transport, HTTP or invalid-response failures.
pub async fn discover_vllm_models(
    base_url: &str,
    api_key: Option<&str>,
    options: &DiscoveryOptions,
) -> Result<Vec<Model>, DiscoveryError> {
    let result = completion_models(DiscoveryProvider::Vllm, base_url, api_key, options).await;
    if let Err(error) = &result {
        (options.log)(DiscoveryDiagnostic::Failed(error));
    }
    result
}

/// Normalize an Ollama request host while keeping the authored model base separate.
fn ollama_host(host: &str) -> Result<String, DiscoveryError> {
    if host.is_empty() {
        return Ok("http://127.0.0.1:11434".into());
    }
    let explicit = host.contains("://") || host.starts_with(':');
    let input = if host.starts_with(':') {
        format!("http://127.0.0.1{host}")
    } else if explicit {
        host.to_owned()
    } else {
        format!("http://{host}")
    };
    let parsed = url::Url::parse(&input).map_err(|error| DiscoveryError {
        provider: DiscoveryProvider::Ollama,
        kind: DiscoveryErrorKind::InvalidUrl,
        message: error.to_string(),
    })?;
    let hostname = parsed.host_str().ok_or_else(|| DiscoveryError {
        provider: DiscoveryProvider::Ollama,
        kind: DiscoveryErrorKind::InvalidUrl,
        message: "Missing host".into(),
    })?;
    let port = parsed.port().unwrap_or(if explicit {
        if parsed.scheme() == "https" { 443 } else { 80 }
    } else {
        11434
    });
    let auth = if parsed.username().is_empty() {
        String::new()
    } else {
        let password = parsed
            .password()
            .map_or_else(String::new, |password| format!(":{password}"));
        format!("{}{password}@", parsed.username())
    };
    let path = parsed.path().strip_suffix('/').unwrap_or(parsed.path());
    Ok(format!(
        "{}://{auth}{hostname}:{port}{path}",
        parsed.scheme()
    ))
}
/// Request an Ollama JSON operation using the shared HTTP effect.
async fn ollama_request(
    host: &str,
    name: Option<&str>,
    options: &DiscoveryOptions,
) -> Result<Members, DiscoveryError> {
    let body = name
        .map(|name| serde_json::to_vec(&HashMap::from([("model", name)])))
        .transpose()
        .map_err(|error| invalid(DiscoveryProvider::Ollama, error))?
        .unwrap_or_default();
    let mut request = HttpRequest {
        method: if name.is_some() { "POST" } else { "GET" }.into(),
        url: format!(
            "{host}/api/{}",
            if name.is_some() { "show" } else { "tags" }
        ),
        headers: [("content-type".into(), "application/json".into())]
            .into_iter()
            .collect(),
        body,
        signal: None,
    };
    request
        .headers
        .insert("accept".into(), "application/json".into());
    http(DiscoveryProvider::Ollama, request, options).await
}
/// Read the selected architecture's decimal context prefix.
fn ollama_context(info: &Members) -> Result<f64, serde_json::Error> {
    let architecture: String = match info
        .get("general.architecture")
        .filter(|v| maestro_json::is_raw_truthy(v))
    {
        Some(raw) => serde_json::from_str(raw.get())?,
        None => String::new(),
    };
    let key = format!("{architecture}.context_length");
    let Some(raw) = info.get(&key).filter(|v| maestro_json::is_raw_truthy(v)) else {
        return Ok(8192.0);
    };
    let text = if raw.get().starts_with('"') {
        serde_json::from_str::<String>(raw.get())?
    } else {
        ryu_js::Buffer::new()
            .format(serde_json::from_str::<f64>(raw.get())?)
            .to_owned()
    };
    super::parse_decimal_prefix(&text).map_err(serde::de::Error::custom)
}
/// Convert a single detail only after tool admission.
async fn ollama_detail(
    name: String,
    host: &str,
    base: &str,
    options: &DiscoveryOptions,
) -> Option<Model> {
    let result = async {
        let object = ollama_request(host, Some(&name), options).await?;
        let capabilities: Vec<String> = field::<Option<Vec<String>>>(&object, "capabilities")
            .map_err(|error| invalid(DiscoveryProvider::Ollama, error))?
            .unwrap_or_default();
        if !capabilities.iter().any(|capability| capability == "tools") {
            (options.log)(DiscoveryDiagnostic::SkippedWithoutTools { model: &name });
            return Ok(None);
        }
        let info = match object.get("model_info").filter(|raw| raw.get() != "null") {
            Some(raw) => {
                members(raw.get()).map_err(|error| invalid(DiscoveryProvider::Ollama, error))?
            }
            None => Members::new(),
        };
        let context =
            ollama_context(&info).map_err(|error| invalid(DiscoveryProvider::Ollama, error))?;
        let maximum = context * 10.0;
        if !context.is_finite() || !maximum.is_finite() {
            return Err(invalid(
                DiscoveryProvider::Ollama,
                "Nonfinite context length",
            ));
        }
        let reasoning = capabilities
            .iter()
            .any(|capability| capability == "thinking");
        Ok(Some(model(
            ModelNames::same(name.clone()),
            base,
            context,
            maximum,
            reasoning,
        )))
    }
    .await;
    match result {
        Ok(model) => model,
        Err(error) => {
            (options.log)(DiscoveryDiagnostic::DetailFailed {
                model: &name,
                error: &error,
            });
            None
        }
    }
}
/// List tool-capable Ollama models, omitting independently failed details.
///
/// # Errors
/// Returns provider-attributed host, listing, HTTP or transport failures.
pub async fn discover_ollama_models(
    base_url: &str,
    _api_key: Option<&str>,
    options: &DiscoveryOptions,
) -> Result<Vec<Model>, DiscoveryError> {
    let result = async {
        let host = ollama_host(base_url)?;
        let object = ollama_request(&host, None, options).await?;
        let rows: Vec<Box<RawValue>> =
            field(&object, "models").map_err(|error| invalid(DiscoveryProvider::Ollama, error))?;
        let names: Vec<String> = rows
            .into_iter()
            .map(|row| {
                let object = members(row.get())
                    .map_err(|error| invalid(DiscoveryProvider::Ollama, error))?;
                field(&object, "name").map_err(|error| invalid(DiscoveryProvider::Ollama, error))
            })
            .collect::<Result<_, _>>()?;
        Ok(futures_util::future::join_all(
            names
                .into_iter()
                .map(|name| ollama_detail(name, &host, base_url, options)),
        )
        .await
        .into_iter()
        .flatten()
        .collect())
    }
    .await;
    if let Err(error) = &result {
        (options.log)(DiscoveryDiagnostic::Failed(error));
    }
    result
}

/// Dispatch a local model discovery operation.
///
/// # Errors
/// Returns the selected provider's discovery failure.
pub async fn discover_models(
    provider: DiscoveryProvider,
    base_url: &str,
    api_key: Option<&str>,
    options: &DiscoveryOptions,
) -> Result<Vec<Model>, DiscoveryError> {
    match provider {
        DiscoveryProvider::Ollama => discover_ollama_models(base_url, api_key, options).await,
        DiscoveryProvider::LlamaCpp => discover_llama_cpp_models(base_url, api_key, options).await,
        DiscoveryProvider::Vllm => discover_vllm_models(base_url, api_key, options).await,
        DiscoveryProvider::Lmstudio => discover_lm_studio_models(base_url, api_key, options).await,
    }
}

mod lm_studio;
pub use lm_studio::discover_lm_studio_models;
