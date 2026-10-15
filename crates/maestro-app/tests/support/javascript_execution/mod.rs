//! Strict recorded sandbox inputs and public-boundary replay.
use maestro_app::SandboxFile;
use maestro_app::browser::{
    components::{sandbox::sandbox_runtime_provider::SandboxRuntimeProvider, sandboxed_iframe::*},
    tools::javascript_repl::*,
};
use maestro_models::{BoxFuture, Cancellation, DiagnosticErrorInfo};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

/// Complete corpus sections, each consumed by its owning witness.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Corpus {
    /// Tool declaration.
    pub metadata: Value,
    /// Existing validator observations.
    pub argument_validation: indexmap::IndexMap<String, Value>,
    /// Complete prompt observations.
    pub descriptions: Vec<Value>,
    /// External mutable descriptions.
    pub dynamic: Value,
    /// Two requests in one run.
    pub between_turns: Value,
    /// Fallible callbacks.
    pub callback_errors: Vec<Value>,
    /// Registered settings observations.
    pub configuration: Value,
    /// Complete execution rows.
    pub cases: Vec<Case>,
}
/// Read the retained independent corpus.
pub fn corpus() -> Corpus {
    serde_json::from_str(include_str!(
        "../../fixtures/javascript_execution/fixtures.json"
    ))
    .unwrap()
}
/// One execution row.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    /// Stable case label.
    pub id: String,
    /// Assigned witness.
    pub test: String,
    /// Complete owned inputs.
    pub input: Input,
    /// Registered result and effects.
    pub expected: Value,
    /// Direct result and effects.
    pub direct: Value,
}
/// Host arguments and outcome.
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Input {
    /// Typed title.
    pub title: String,
    /// Execution call identity.
    pub tool_call_id: String,
    /// Authored code.
    pub code: String,
    /// Optional cancellation state.
    pub signal: Option<Value>,
    /// Unset factory.
    pub factory_missing: bool,
    /// Ordered provider identities.
    pub runtime_provider_ids: Vec<String>,
    /// Optional URL callback value.
    pub sandbox_url_provider: Option<String>,
    /// Successful host return.
    pub result: Option<Outcome>,
    /// Rejected host operation.
    pub throw_message: Option<String>,
}
/// Typed host result; the raw return member preserves null versus absence.
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Outcome {
    /// Success discriminant.
    pub success: bool,
    /// Ordered console entries.
    pub console: Vec<Console>,
    /// Optional downloads.
    pub files: Option<Vec<Download>>,
    /// Optional error text.
    pub error: Option<Failure>,
    /// Present-null remains present.
    #[serde(default, deserialize_with = "present")]
    pub return_value: Option<Value>,
}
/// Preserve explicit null.
fn present<'de, D: serde::Deserializer<'de>>(decoder: D) -> Result<Option<Value>, D::Error> {
    Value::deserialize(decoder).map(Some)
}
/// Console record.
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Console {
    /// Open category.
    pub r#type: String,
    /// Captured text.
    pub text: String,
}
/// Failure record.
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Failure {
    /// Error message.
    pub message: String,
    /// Optional stack.
    #[serde(default)]
    pub stack: String,
}
/// Download fixture.
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Download {
    /// Original name.
    pub file_name: String,
    /// Original MIME.
    pub mime_type: String,
    /// Text or compact lossless binary sample.
    pub content: Value,
}
impl Download {
    /// Expand the recorded payload.
    pub fn owned(&self) -> SandboxFile {
        let content = if let Some(text) = self.content.as_str() {
            SandboxFileContent::Text(text.into())
        } else if let Some(bytes) = self.content.get("bytes") {
            SandboxFileContent::Bytes(serde_json::from_value(bytes.clone()).unwrap())
        } else {
            let n = self.content["sequenceBytes"].as_u64().unwrap();
            SandboxFileContent::Bytes((0..n).map(|i| u8::try_from(i % 256).unwrap()).collect())
        };
        SandboxFile {
            file_name: self.file_name.clone(),
            mime_type: self.mime_type.clone(),
            content,
        }
    }
}
impl Outcome {
    /// Construct the caller-owned result.
    pub fn owned(&self) -> SandboxResult {
        SandboxResult {
            success: self.success,
            console: self
                .console
                .iter()
                .map(|entry| SandboxConsoleEntry {
                    r#type: entry.r#type.clone(),
                    text: entry.text.clone(),
                })
                .collect(),
            files: self
                .files
                .as_ref()
                .map(|files| files.iter().map(Download::owned).collect()),
            error: self.error.as_ref().map(|error| SandboxError {
                message: error.message.clone(),
                stack: error.stack.clone(),
            }),
            return_value: self.return_value.clone().map(decode_return),
        }
    }
}
/// Provider with distinguishable data and description.
pub struct Provider(pub String);
impl SandboxRuntimeProvider for Provider {
    fn get_data(&self) -> Result<indexmap::IndexMap<String, Value>, DiagnosticErrorInfo> {
        Ok([("id".into(), json!(self.0))].into())
    }
    fn get_runtime(&self) -> Result<String, DiagnosticErrorInfo> {
        Ok("function(){ return 42; }".into())
    }
    fn get_description(&self) -> Result<String, DiagnosticErrorInfo> {
        Ok(self.0.clone())
    }
}
/// Shared public-port observations, not production introspection.
#[derive(Default)]
pub struct Observations {
    /// Ordered host effects.
    pub events: Vec<String>,
    /// Removed sandbox count.
    pub removals: usize,
    /// Execution identifiers.
    pub ids: Vec<String>,
    /// Factory evaluations.
    pub factory_calls: usize,
}
/// Host constructed from one complete recorded row.
pub struct Host {
    /// Input supplied by the independent recording.
    pub input: Input,
    /// Retained effect transcript.
    pub observed: Arc<Mutex<Observations>>,
}
impl SandboxHost for Host {
    fn create_hidden(
        &self,
        url: Option<SandboxUrlProvider>,
    ) -> Result<Box<dyn SandboxIframe>, DiagnosticErrorInfo> {
        assert_eq!(
            url.as_ref().map(|url| url().unwrap()),
            self.input.sandbox_url_provider
        );
        self.observed
            .lock()
            .unwrap()
            .events
            .push("create_hidden".into());
        Ok(Box::new(Self {
            input: self.input.clone(),
            observed: Arc::clone(&self.observed),
        }))
    }
}
impl SandboxIframe for Host {
    fn execute(
        &self,
        request: SandboxExecution,
    ) -> BoxFuture<Result<SandboxResult, DiagnosticErrorInfo>> {
        assert_eq!(request.code, self.input.code);
        assert_eq!(
            request.signal.as_ref().map(Cancellation::is_aborted),
            self.input
                .signal
                .as_ref()
                .map(|signal| signal["aborted"].as_bool().unwrap())
        );
        let ids: Vec<_> = request
            .runtime_providers
            .iter()
            .map(|provider| {
                provider.get_data().unwrap()["id"]
                    .as_str()
                    .unwrap()
                    .to_owned()
            })
            .collect();
        assert_eq!(ids, self.input.runtime_provider_ids);
        {
            let mut observed = self.observed.lock().unwrap();
            observed.events.push("execute".into());
            assert!(request.sandbox_id.starts_with("repl-"));
            observed.ids.push(request.sandbox_id);
        }
        let result = self
            .input
            .result
            .as_ref()
            .map(Outcome::owned)
            .ok_or_else(|| DiagnosticErrorInfo::error(self.input.throw_message.clone().unwrap()));
        Box::pin(async move { result })
    }
    fn remove(&self) {
        let mut observed = self.observed.lock().unwrap();
        observed.events.push("remove".into());
        observed.removals += 1;
    }
}
/// Build ordered supplied providers.
pub fn providers(ids: &[String]) -> Vec<Arc<dyn SandboxRuntimeProvider>> {
    ids.iter()
        .map(|id| Arc::new(Provider(id.clone())) as Arc<dyn SandboxRuntimeProvider>)
        .collect()
}
/// Construct a signal without flattening absent and present.
pub fn signal(input: &Input) -> Option<Cancellation> {
    input.signal.as_ref().map(|state| {
        let signal = Cancellation::new();
        if state["aborted"] == true {
            signal.abort();
        }
        signal
    })
}
/// Run every assigned direct execution and compare the complete outcome.
pub async fn direct_rows(test: &str) {
    let cases: Vec<_> = corpus()
        .cases
        .into_iter()
        .filter(|case| case.test == test)
        .collect();
    assert!(!cases.is_empty());
    for case in cases {
        let observed = Arc::new(Mutex::new(Observations::default()));
        let host = Arc::new(Host {
            input: case.input.clone(),
            observed: Arc::clone(&observed),
        });
        let environment = SandboxEnvironment {
            host,
            sandbox_url_provider: case
                .input
                .sandbox_url_provider
                .clone()
                .map(|url| Arc::new(move || Ok(url.clone())) as SandboxUrlProvider),
        };
        let result = execute_javascript(
            case.input.code.clone(),
            providers(&case.input.runtime_provider_ids),
            signal(&case.input),
            environment,
        )
        .await;
        match result {
            Ok(result) => {
                assert_eq!(case.direct["branch"], "success", "{}", case.id);
                assert_eq!(
                    result.output, case.direct["result"]["output"],
                    "{}",
                    case.id
                );
                let expected = case.direct["result"].get("files").map(|files| {
                    serde_json::from_value::<Vec<Download>>(files.clone())
                        .unwrap()
                        .iter()
                        .map(Download::owned)
                        .collect::<Vec<_>>()
                });
                assert_eq!(result.files, expected, "{}", case.id);
            }
            Err(error) => {
                assert_eq!(case.direct["branch"], "error", "{}", case.id);
                assert_eq!(error.message, case.direct["message"], "{}", case.id);
            }
        }
        let seen = observed.lock().unwrap();
        assert_eq!(
            json!({"events":seen.events,"removals":seen.removals}),
            case.direct["effects"],
            "{}",
            case.id
        );
    }
}

/// Replay the registered typed route alongside its direct route.
pub async fn rows(test: &str) {
    direct_rows(test).await;
    for case in corpus().cases.into_iter().filter(|case| case.test == test) {
        let observed = Arc::new(Mutex::new(Observations::default()));
        let repl = maestro_app::create_javascript_repl_tool(Arc::new(Host {
            input: case.input.clone(),
            observed: Arc::clone(&observed),
        }));
        let ids = case.input.runtime_provider_ids.clone();
        let seen = Arc::clone(&observed);
        *repl.runtime_providers_factory.write().unwrap() =
            (!case.input.factory_missing).then(|| {
                Arc::new(move || {
                    seen.lock().unwrap().factory_calls += 1;
                    Ok(providers(&ids))
                }) as RuntimeProvidersFactory<Value>
            });
        *repl.sandbox_url_provider.write().unwrap() = case
            .input
            .sandbox_url_provider
            .clone()
            .map(|url| Arc::new(move || Ok(url.clone())) as SandboxUrlProvider);
        let execute = Arc::clone(&repl.tool.read().unwrap().execute);
        let result = execute(
            case.input.tool_call_id.clone(),
            json!({"title":case.input.title,"code":case.input.code}),
            signal(&case.input),
            None,
        )
        .await;
        match result {
            Ok(result) => {
                assert_eq!(case.expected["branch"], "success", "{}", case.id);
                assert_eq!(
                    json!({"content":result.content,"details":result.details}),
                    case.expected["result"],
                    "{}",
                    case.id
                );
            }
            Err(error) => {
                assert_eq!(case.expected["branch"], "error", "{}", case.id);
                assert_eq!(error.message, case.expected["message"], "{}", case.id);
            }
        }
        let seen = observed.lock().unwrap();
        assert_eq!(
            json!({"events":seen.events,"removals":seen.removals,"factoryCalls":seen.factory_calls}),
            case.expected["effects"],
            "{}",
            case.id
        );
    }
}

/// Restore the lossless negative-zero marker before host publication.
fn decode_return(value: Value) -> Value {
    match value {
        Value::Object(ref object)
            if object.len() == 1 && object.get("negativeZero") == Some(&Value::Bool(true)) =>
        {
            json!(-0.0)
        }
        Value::Object(object) => Value::Object(
            object
                .into_iter()
                .map(|(key, value)| (key, decode_return(value)))
                .collect(),
        ),
        Value::Array(array) => Value::Array(array.into_iter().map(decode_return).collect()),
        value => value,
    }
}
