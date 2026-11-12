//! Browser transactions and source-derived raw-value fixture construction.
use maestro_app::Runtime;
use maestro_app::browser::components::sandbox::providers::SandboxRuntimeProvider;
use maestro_models::DiagnosticErrorInfo;
use maestro_models::SharedHandle;
use maestro_web::browser::values::BrowserSandboxValue;
use maestro_web::components::sandboxed_iframe::PrepareHtmlOptions;
use wasm_bindgen::{JsCast as _, prelude::*};

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
    /// Fixture namespace used by the native message observer.
    pub fixture: Fixture,
    /// Self-contained function source.
    pub runtime: String,
    /// Reached provider failure phase.
    pub failure: Option<String>,
}
impl SandboxRuntimeProvider<BrowserSandboxValue> for Provider {
    fn handle_message(
        &self,
        message: BrowserSandboxValue,
        respond: maestro_app::browser::components::sandbox::providers::SandboxRespond<
            BrowserSandboxValue,
        >,
    ) -> maestro_models::BoxFuture<Result<(), DiagnosticErrorInfo>> {
        if js_sys::Reflect::get(&message.0, &"type".into())
            .unwrap()
            .as_string()
            .as_deref()
            == Some("execution-error")
        {
            self.fixture.record_provider(&message.0);
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
    fn get_runtime(&self) -> Result<Runtime, DiagnosticErrorInfo> {
        if self.failure.as_deref() == Some("runtime") {
            return Err(DiagnosticErrorInfo::error("provider failed"));
        }
        Ok(Runtime::Content(self.runtime.clone()))
    }
    fn get_description(&self) -> Result<String, DiagnosticErrorInfo> {
        Ok(String::new())
    }
}
pub(crate) mod placement;
pub(crate) use placement::{adapter, adapter_pack};
/// Materialize native provider values before preparation, preserving failure phases.
#[must_use]
pub(crate) fn document_cases(fixture: &Fixture) -> Vec<DocumentCase> {
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
                            fixture: fixture.clone(),
                            data: native::decode(
                                &js_sys::JSON::parse(&p["data"].to_string()).unwrap(),
                            ),
                            runtime: p["runtime"].as_str().unwrap().into(),
                            failure: None,
                        })
                            as SharedHandle<dyn SandboxRuntimeProvider<BrowserSandboxValue>>
                    })
                    .collect();
            if let Some(phase) = input["provider_failure"].as_str() {
                providers.push(SharedHandle::new(Provider {
                    fixture: fixture.clone(),
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
#[must_use]
pub(crate) fn provider(
    fixture: &Fixture,
    source: &str,
) -> SharedHandle<dyn SandboxRuntimeProvider<BrowserSandboxValue>> {
    SharedHandle::new(Provider {
        fixture: fixture.clone(),
        data: js_sys::Object::new().into(),
        runtime: source.into(),
        failure: None,
    })
}

/// Selected behavioral fixture with no discarded envelope fields.
pub struct Observation {
    /// Complete input driving this observation.
    pub input: serde_json::Value,
    /// Expected observable output.
    pub output: serde_json::Value,
}
/// Select distinct callback or navigation inputs; names select, inputs drive, outputs assert.
#[must_use]
pub(crate) fn fixture_group(prefix: &str) -> Vec<Observation> {
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

/// Consumer observing actual registered dispatch rather than adapter bookkeeping.
pub struct Consumer(pub Fixture);
impl maestro_web::components::sandbox::runtime_message_router::MessageConsumer<BrowserSandboxValue>
    for Consumer
{
    fn handle_message(
        &self,
        message: BrowserSandboxValue,
    ) -> maestro_models::BoxFuture<Result<(), DiagnosticErrorInfo>> {
        self.0.record(&message.0);
        Box::pin(async { Ok(()) })
    }
}

/// Mounted program and effective-target transactions.
pub mod programs;

/// Page-side observations of prepared documents and their programs.
pub(crate) mod native;
/// Native message registrations, openers and frame routes.
pub(crate) mod routes;

/// One browser loading transaction: prepared-document observations and frame routes.
#[derive(Clone, Default)]
pub struct Fixture {
    /// The routing transaction below the adapter.
    routes: routes::Routes,
}
impl Fixture {
    /// Compare generated documents and execute their parser-restored programs.
    pub(crate) async fn observe_document(name: &str, html: &str, output: &JsValue) {
        native::observe_document(name, html, output).await;
    }
    /// Restore instrumentation and remove all test parents.
    pub(crate) fn finish() {
        native::finish();
    }
    /// Names defined on a window after the document is written into a fresh frame, by pattern.
    pub(crate) async fn defined(html: &str, pattern: &str, helpers: &[&str]) -> JsValue {
        native::defined(html, pattern, helpers).await
    }
    /// Observe native registration synchronously around standalone runtime execution.
    pub(crate) async fn standalone(html: &str) {
        native::standalone(html).await;
    }
    /// Execute the wrapper with controlled ordered completion gates.
    pub(crate) async fn callbacks(
        html: &str,
        input: &serde_json::Value,
        expected: &serde_json::Value,
    ) {
        native::callbacks(html, input, expected).await;
    }
    /// Instrument native registrations and openers below the adapter.
    pub(crate) fn begin(&self) {
        self.routes.begin();
    }
    /// Start waiting before the frame is mounted or signalled.
    pub(crate) fn arm(&self, kind: &str) {
        self.routes.arm(kind);
    }
    /// Await the exact producer completion message.
    pub(crate) async fn waited(&self) -> JsValue {
        self.routes.waited().await
    }
    /// Exercise native link clicks and assert routing after each completed load.
    pub(crate) async fn route_check(&self, mode: &str) {
        self.routes.route_check(mode).await;
    }
    /// Post a controlled current-source message synchronously through native dispatch.
    pub(crate) fn dispatch(&self, kind: &str, source: &str) {
        self.routes.dispatch(kind, source);
    }
    /// Observe synchronous detach and prove closed native routes cannot deliver.
    pub(crate) fn closed(&self, expected: u32) {
        self.routes.closed(expected);
    }
    /// Return the authored load probe document.
    #[must_use]
    pub(crate) fn content() -> String {
        routes::content().to_owned()
    }
    /// Build a URL bootstrap with a controlled handshake.
    #[must_use]
    pub(crate) fn url(mode: &str) -> String {
        routes::url(mode)
    }
    /// Record the same consumer payload received through normal routing.
    pub(crate) fn record(&self, message: &JsValue) {
        self.routes.record(message);
    }
    /// Retain the provider's observation independently of the consumer.
    pub(crate) fn record_provider(&self, message: &JsValue) {
        self.routes.record_provider(message);
    }
    /// Configure one controlled failure below the adapter.
    pub(crate) fn fault(&self, mode: &str) {
        self.routes.fault(mode);
    }
    /// Restore instrumentation after native cleanup.
    pub(crate) fn end(&self) {
        self.routes.end();
    }
    /// Dispatch native events through the injected navigation handler.
    pub(crate) async fn navigation(
        html: &str,
        input: &serde_json::Value,
        expected: &serde_json::Value,
    ) {
        native::navigation(html, input, expected).await;
    }
    /// Observe the mounted native validation document, or accepted recovered input.
    pub(crate) async fn validation_check(&self, expected: &str, css: &str) {
        self.routes.validation_check(expected, css).await;
    }
}

/// A provider selecting one built-in helper set.
pub struct Builtin(pub maestro_app::BuiltinRuntime);
impl SandboxRuntimeProvider<BrowserSandboxValue> for Builtin {
    fn get_data(
        &self,
    ) -> Result<indexmap::IndexMap<String, BrowserSandboxValue>, DiagnosticErrorInfo> {
        Ok(indexmap::IndexMap::new())
    }
    fn get_runtime(&self) -> Result<Runtime, DiagnosticErrorInfo> {
        Ok(Runtime::Builtin(self.0))
    }
    fn get_description(&self) -> Result<String, DiagnosticErrorInfo> {
        Ok(String::new())
    }
}
