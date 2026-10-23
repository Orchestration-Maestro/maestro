use maestro_agent::{AgentToolResult, ExecuteTool};
use maestro_app::{DiagnosticErrorInfo, Fetch, FetchError, HttpResponse};
use maestro_models::{Cancellation, SharedHandle};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex, RwLock, Weak};

/// Complete recorded tool observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Corpus {
    /// Declaration and identity expectations.
    pub metadata: Metadata,
    /// Distinct execution cases.
    pub cases: Vec<Case>,
}
/// One recorded input and its complete transcript.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    /// Diagnostic case label.
    pub name: String,
    /// Supplied operands.
    pub input: Input,
    /// Full expected outcome and effects.
    pub expected: Value,
}
/// Supplied execution configuration.
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Input {
    /// Authored URL.
    pub url: String,
    /// Already-aborted signal.
    #[serde(default)]
    pub pre_aborted: bool,
    /// Direct response.
    #[serde(default)]
    pub response: Response,
    /// Initial proxy setting.
    pub proxy: Option<String>,
    /// Replacement during direct fetch.
    pub proxy_changed_in_fetch: Option<String>,
    /// Direct transport error.
    pub direct_error: Option<Error>,
    /// Proxy transport error.
    pub proxy_error: Option<Error>,
    /// Proxy response.
    #[serde(default)]
    pub proxy_response: Response,
    /// Supplied attachment projection.
    #[serde(default)]
    pub attachment: Value,
    /// Loader error.
    pub load_error: Option<String>,
    /// Use the production loader.
    #[serde(default)]
    pub real_attachment: bool,
    /// Acknowledged pending phase.
    pub phase: Option<String>,
}
/// Controlled response operands.
#[derive(Clone, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Response {
    /// HTTP status override.
    pub status: Option<u16>,
    /// HTTP reason override.
    pub status_text: Option<String>,
    /// Declared size.
    pub content_length: Option<String>,
    /// Header MIME deliberately independent of attachment selection.
    pub content_type: Option<String>,
    /// Body allocation length.
    pub body_size: Option<usize>,
    /// Actual text bytes.
    pub body_text: Option<String>,
    /// Body failure.
    pub body_error: Option<String>,
}
/// Controlled diagnostic preserves producer fields.
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Error {
    /// Diagnostic name.
    pub name: String,
    /// Diagnostic text.
    pub message: String,
}
impl Error {
    /// Convert without discarding supplied diagnostic fields.
    pub fn diagnostic(&self) -> DiagnosticErrorInfo {
        DiagnosticErrorInfo {
            name: Some(self.name.clone()),
            message: self.message.clone(),
            stack: None,
            code: None,
        }
    }
}
/// Observable effects retained outside the executable.
#[derive(Default)]
pub struct Effects {
    /// Authored fetch operands and shared signal ownership.
    pub requests: Vec<(String, Cancellation)>,
    /// Ordered body reads.
    pub bodies: Vec<String>,
    /// Ordered attachment operands.
    pub loads: Vec<Value>,
    /// Non-owning access to a setting changed inside the selected adapter.
    pub proxy: Weak<RwLock<Option<String>>>,
}
/// Parse the strict consumer corpus.
pub fn corpus() -> Corpus {
    serde_json::from_str(include_str!(
        "../fixtures/document_extraction/fixtures.json"
    ))
    .unwrap()
}
/// Selected controlled transport.
pub fn fetch(input: &Input, effects: &Arc<Mutex<Effects>>) -> Fetch {
    let input = input.clone();
    let effects = Arc::clone(effects);
    Arc::new(move |request| {
        assert_eq!(request.method, "GET");
        assert!(request.headers.is_empty() && request.body.is_empty());
        let (direct, proxy) = {
            let mut effects = effects.lock().unwrap();
            let direct = effects.requests.is_empty();
            effects
                .requests
                .push((request.url, request.signal.unwrap()));
            (direct, effects.proxy.upgrade())
        };
        if direct && let Some(changed) = &input.proxy_changed_in_fetch {
            *proxy.unwrap().write().unwrap() = Some(changed.clone());
        }
        let error = if direct {
            input.direct_error.as_ref()
        } else {
            input.proxy_error.as_ref()
        };
        let response = if direct {
            input.response.clone()
        } else {
            input.proxy_response.clone()
        };
        let effects = Arc::clone(&effects);
        let error = error.map(Error::diagnostic);
        let phase = if direct { "direct" } else { "proxy" };
        Box::pin(async move {
            error.map_or_else(
                || Ok(response.make(effects, phase)),
                |error| Err(FetchError::Connection(error)),
            )
        })
    })
}
impl Response {
    /// Build a body whose effects occur only when polled.
    pub fn make(self, effects: Arc<Mutex<Effects>>, phase: &'static str) -> HttpResponse {
        let mut headers = std::collections::BTreeMap::new();
        if let Some(length) = self.content_length {
            headers.insert("content-length".into(), length);
        }
        if let Some(mime) = self.content_type {
            headers.insert("content-type".into(), mime);
        }
        HttpResponse {
            status: self.status.unwrap_or(200),
            status_text: self.status_text.unwrap_or_else(|| "OK".into()),
            headers,
            body: Box::pin(futures_util::stream::once(async move {
                read_body(
                    self.body_text,
                    self.body_size,
                    self.body_error,
                    &effects,
                    phase,
                )
            })),
        }
    }
}
/// Normalize only the common error/result envelope.
pub fn outcome(result: Result<AgentToolResult, DiagnosticErrorInfo>) -> Value {
    match result {
        Ok(result) => json!({"result":result}),
        Err(error) => {
            json!({"error":{"name":error.name.unwrap_or_else(||"Error".into()),"message":error.message}})
        }
    }
}
/// Invoke the retained callback and compare every recorded effect.
pub async fn check(case: Case, execute: ExecuteTool, effects: Arc<Mutex<Effects>>) {
    let signal = Cancellation::new();
    if case.input.pre_aborted {
        signal.abort();
    }
    let mut actual = outcome(
        execute(
            String::new(),
            json!({"url":case.input.url}),
            Some(signal.clone()),
            Some(SharedHandle::new(|_| panic!("unexpected progress update"))),
        )
        .await,
    );
    signal.abort();
    let effects = effects.lock().unwrap();
    actual = transcript(actual, &effects);
    if case.input.real_attachment {
        assert!(effects.loads.is_empty());
        actual.as_object_mut().unwrap().shift_remove("loads");
    }
    assert_eq!(actual, case.expected, "case {}", case.name);
}

/// Poll-visible body effect and selected bytes or failure.
fn read_body(
    text: Option<String>,
    size: Option<usize>,
    error: Option<String>,
    effects: &Mutex<Effects>,
    phase: &'static str,
) -> Result<Vec<u8>, FetchError> {
    effects.lock().unwrap().bodies.push(phase.into());
    if let Some(error) = error {
        return Err(FetchError::Connection(DiagnosticErrorInfo::error(error)));
    }
    Ok(text.map_or_else(|| vec![0; size.unwrap_or(17)], String::into_bytes))
}

/// Strict declaration and alias expectations.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Metadata {
    /// Tool label.
    label: String,
    /// Model-facing name.
    name: String,
    /// Model-facing description.
    description: String,
    /// Ordered parameter schema, compared in full.
    parameters: Value,
    /// Initial setting absence.
    proxy_initially_absent: RecordedFlag,
    /// Independent declarations.
    factories_are_distinct: RecordedFlag,
    /// Existing default instance.
    default_is_preconstructed: RecordedFlag,
    /// Shared default declaration.
    default_is_shared: RecordedFlag,
    /// Independent setting slots.
    factory_settings_independent: RecordedFlag,
    /// Retained registered alias.
    default_retains_mutable_registration_alias: RecordedFlag,
}
impl Metadata {
    /// Serialize the strict record for full metadata assertions.
    pub fn value(self) -> Value {
        serde_json::to_value(self).unwrap()
    }
}

/// A recorded boolean observation independent of runtime configuration.
#[derive(Deserialize, Serialize)]
#[serde(transparent)]
struct RecordedFlag(bool);

/// Attach the complete ordered effect transcript after execution settles.
pub fn transcript(mut actual: Value, effects: &Effects) -> Value {
    actual["requests"] = effects
        .requests
        .iter()
        .map(|(url, captured)| {
            assert!(captured.is_aborted(), "signal did not share cancellation");
            json!({"url":url,"signalSame":true})
        })
        .collect();
    actual["bodyReads"] = json!(effects.bodies);
    actual["loads"] = json!(effects.loads);
    actual
}
