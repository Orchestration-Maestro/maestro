//! Browser transactions and source-derived raw-value fixture construction.
use maestro_app::DiagnosticErrorInfo;
use maestro_app::SharedHandle;
use maestro_app::browser::components::sandbox::sandbox_runtime_provider::SandboxRuntimeProvider;
use maestro_web::browser::sandbox_value::BrowserSandboxValue;
use maestro_web::components::sandboxed_iframe::PrepareHtmlOptions;
use wasm_bindgen::{JsCast as _, prelude::*};

#[wasm_bindgen(module = "/tests/sandbox_loading_support/native.js")]
extern "C" {
    /// Decode special native values from the recorded input.
    fn decode(value: JsValue) -> JsValue;
    /// Compare generated documents and execute their parser-restored programs.
    pub fn observe_document(name: &str, html: &str, output: JsValue) -> js_sys::Promise;
    /// Restore instrumentation and remove all test parents.
    pub fn finish();
}
/// Exact checked-in oracle envelope; every field is consumed.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Recorded case label.
    name: String,
    /// Complete source-derived operation input.
    input: serde_json::Value,
    /// Consumed source-derived observations.
    output: serde_json::Value,
}
/// One public preparation invocation with its expected output.
pub struct DocumentCase {
    /// Recorded case label.
    pub name: String,
    /// Supplied sandbox identity.
    pub id: String,
    /// Unchanged authored input.
    pub code: String,
    /// Retained provider allocations.
    pub providers: Vec<SharedHandle<dyn SandboxRuntimeProvider<BrowserSandboxValue>>>,
    /// Independent envelope and standalone selection.
    pub options: PrepareHtmlOptions,
    /// Complete consumed oracle output.
    pub expected: JsValue,
}
/// Self-contained runtime source and native retained data.
pub struct Provider {
    /// Native provider data including non-JSON values.
    pub data: JsValue,
    /// Self-contained function source.
    pub runtime: String,
    /// Reached provider failure phase.
    pub failure: Option<String>,
}
impl SandboxRuntimeProvider<BrowserSandboxValue> for Provider {
    fn handle_message(
        &self,
        message: BrowserSandboxValue,
        respond: maestro_app::browser::components::sandbox::sandbox_runtime_provider::SandboxRespond<BrowserSandboxValue>,
    ) -> maestro_app::BoxFuture<Result<(), DiagnosticErrorInfo>> {
        if js_sys::Reflect::get(&message.0, &"type".into())
            .unwrap()
            .as_string()
            .as_deref()
            == Some("execution-error")
        {
            record_provider(message.0);
            let result = respond(BrowserSandboxValue(
                js_sys::JSON::parse("{\"bootstrapAcknowledged\":true}").unwrap(),
            ));
            return Box::pin(async { result });
        }
        Box::pin(async { Ok(()) })
    }
    fn on_execution_end(&self, _: &str) -> Result<(), DiagnosticErrorInfo> {
        panic!("loading must not invoke execution end")
    }

    fn get_data(
        &self,
    ) -> Result<indexmap::IndexMap<String, BrowserSandboxValue>, DiagnosticErrorInfo> {
        if self.failure.as_deref() == Some("data") {
            return Err(DiagnosticErrorInfo::error("provider failed"));
        }
        js_sys::Object::keys(self.data.unchecked_ref::<js_sys::Object>())
            .iter()
            .map(|key| {
                Ok((
                    key.as_string().unwrap(),
                    BrowserSandboxValue(js_sys::Reflect::get(&self.data, &key).unwrap()),
                ))
            })
            .collect()
    }
    fn get_runtime(&self) -> Result<String, DiagnosticErrorInfo> {
        if self.failure.as_deref() == Some("runtime") {
            return Err(DiagnosticErrorInfo::error("provider failed"));
        }
        Ok(self.runtime.clone())
    }
    fn get_description(&self) -> Result<String, DiagnosticErrorInfo> {
        Ok(String::new())
    }
}
pub(crate) mod placement;
pub use placement::{adapter, adapter_pack};
/// Materialize native provider values before preparation, preserving failure phases.
pub fn document_cases() -> Vec<DocumentCase> {
    let rows: Vec<Case> =
        serde_json::from_str(include_str!("../fixtures/sandbox_documents.json")).unwrap();
    assert_eq!(rows.len(), 54);
    rows.into_iter()
        .filter(|row| row.input.get("id").is_some() || row.input.get("provider_failure").is_some())
        .map(|row| {
            let input = row.input;
            let mut providers: Vec<SharedHandle<dyn SandboxRuntimeProvider<BrowserSandboxValue>>> =
                input["providers"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|p| {
                        SharedHandle::new(Provider {
                            data: decode(js_sys::JSON::parse(&p["data"].to_string()).unwrap()),
                            runtime: p["runtime"].as_str().unwrap().into(),
                            failure: None,
                        })
                            as SharedHandle<dyn SandboxRuntimeProvider<BrowserSandboxValue>>
                    })
                    .collect();
            if let Some(phase) = input["provider_failure"].as_str() {
                providers.push(SharedHandle::new(Provider {
                    data: js_sys::Object::new().into(),
                    runtime: "function(){}".into(),
                    failure: Some(phase.into()),
                }));
            }
            DocumentCase {
                name: row.name,
                id: input["id"].as_str().unwrap_or("provider-failure").into(),
                code: input["code"].as_str().unwrap_or("<head></head>").into(),
                providers,
                options: PrepareHtmlOptions {
                    is_html_artifact: input["options"]["isHtmlArtifact"]
                        .as_bool()
                        .unwrap_or_default(),
                    is_standalone: input["options"]["isStandalone"]
                        .as_bool()
                        .unwrap_or_default(),
                },
                expected: js_sys::JSON::parse(&row.output.to_string()).unwrap(),
            }
        })
        .collect()
}
/// Native callback for a simple self-contained runtime.
pub fn provider(source: &str) -> SharedHandle<dyn SandboxRuntimeProvider<BrowserSandboxValue>> {
    SharedHandle::new(Provider {
        data: js_sys::Object::new().into(),
        runtime: source.into(),
        failure: None,
    })
}

#[wasm_bindgen(module = "/tests/sandbox_loading_support/native.js")]
extern "C" {
    /// Observe native registration synchronously around standalone runtime execution.
    pub fn standalone(html: &str) -> js_sys::Promise;
    /// Execute the wrapper with controlled ordered completion gates.
    pub fn callbacks(html: &str, input: &str, expected: &str) -> js_sys::Promise;
}
/// Selected behavioral fixture with no discarded envelope fields.
pub struct Observation {
    /// Complete input driving this observation.
    pub input: serde_json::Value,
    /// Expected observable output.
    pub output: serde_json::Value,
}
/// Select distinct callback or navigation inputs; names select, inputs drive, outputs assert.
pub fn fixture_group(prefix: &str) -> Vec<Observation> {
    let rows: Vec<Case> =
        serde_json::from_str(include_str!("../fixtures/sandbox_documents.json")).unwrap();
    rows.into_iter()
        .filter(|row| row.name.starts_with(prefix))
        .map(|row| Observation {
            input: row.input,
            output: row.output,
        })
        .collect()
}

#[wasm_bindgen(module = "/tests/sandbox_loading_support/routes.js")]
extern "C" {
    /// Instrument native registrations and openers below the adapter.
    pub fn begin();
    /// Start waiting before the frame is mounted or signalled.
    pub fn arm(kind: &str);
    /// Await the exact producer completion message.
    pub fn waited() -> js_sys::Promise;
    /// Exercise native link clicks and assert routing after each completed load.
    pub fn route_check(mode: &str) -> js_sys::Promise;
    /// Post a controlled current-source message synchronously through native dispatch.
    pub fn dispatch(kind: &str, source: &str);
    /// Observe synchronous detach and prove closed native routes cannot deliver.
    pub fn closed(expected: u32);
    /// Return the authored load probe document.
    pub fn content() -> String;
    /// Build a URL bootstrap with a controlled handshake.
    pub fn url(mode: &str) -> String;
    /// Record the same consumer payload received through normal routing.
    pub fn record(message: JsValue);
    /// Retain the provider's observation independently of the consumer.
    pub fn record_provider(message: JsValue);
    /// Configure one controlled failure below the adapter.
    pub fn fault(mode: &str);
    /// Restore instrumentation after native cleanup.
    pub fn end();
}
/// Consumer observing actual registered dispatch rather than adapter bookkeeping.
pub struct Consumer;
impl maestro_web::components::sandbox::runtime_message_router::MessageConsumer<BrowserSandboxValue>
    for Consumer
{
    fn handle_message(
        &self,
        message: BrowserSandboxValue,
    ) -> maestro_app::BoxFuture<Result<(), DiagnosticErrorInfo>> {
        record(message.0);
        Box::pin(async { Ok(()) })
    }
}

#[wasm_bindgen(module = "/tests/sandbox_loading_support/native.js")]
extern "C" {
    /// Dispatch native events through the injected navigation handler.
    pub fn navigation(html: &str, input: &str, expected: &str) -> js_sys::Promise;
}

#[wasm_bindgen(module = "/tests/sandbox_loading_support/routes.js")]
extern "C" {
    /// Observe the mounted native validation document, or accepted recovered input.
    pub fn validation_check(expected: &str, css: &str) -> js_sys::Promise;
}

/// Mounted program and effective-target transactions.
pub mod programs;
