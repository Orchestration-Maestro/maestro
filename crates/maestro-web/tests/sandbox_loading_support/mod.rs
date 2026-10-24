//! Browser transactions and source-derived raw-value fixture construction.
use maestro_app::DiagnosticErrorInfo;
use maestro_app::SharedHandle;
use maestro_app::browser::components::sandbox::providers::SandboxRuntimeProvider;
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
    ) -> maestro_app::BoxFuture<Result<(), DiagnosticErrorInfo>> {
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
                            data: fixture
                                .decode(&js_sys::JSON::parse(&p["data"].to_string()).unwrap()),
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
    ) -> maestro_app::BoxFuture<Result<(), DiagnosticErrorInfo>> {
        self.0.record(&message.0);
        Box::pin(async { Ok(()) })
    }
}

/// Mounted program and effective-target transactions.
pub mod programs;

/// Native namespaces retained for one browser test transaction.
#[derive(Clone)]
pub struct Fixture {
    /// Original packaged native namespace.
    module0: crate::browser_fixture_support::FixtureModule,
    /// Original packaged native namespace.
    module1: crate::browser_fixture_support::FixtureModule,
}
impl Fixture {
    /// Load each namespace before installing controlled host effects.
    pub(crate) async fn load() -> Self {
        Self {
            module0: crate::browser_fixture_support::FixtureModule::load(&wasm_bindgen::link_to!(
                module = "/tests/sandbox_loading_support/native.js"
            ))
            .await
            .unwrap(),
            module1: crate::browser_fixture_support::FixtureModule::load(&wasm_bindgen::link_to!(
                module = "/tests/sandbox_loading_support/routes.js"
            ))
            .await
            .unwrap(),
        }
    }
    /// Decode special native values from the recorded input.
    #[must_use]
    pub(crate) fn decode(&self, value: &JsValue) -> JsValue {
        self.module0
            .call("decode", &js_sys::Array::of1(value))
            .unwrap()
    }
    /// Compare generated documents and execute their parser-restored programs.
    pub(crate) fn observe_document(
        &self,
        name: &str,
        html: &str,
        output: &JsValue,
    ) -> js_sys::Promise {
        self.module0
            .call(
                "observe_document",
                &js_sys::Array::of3(&name.into(), &html.into(), output),
            )
            .unwrap()
            .unchecked_into()
    }
    /// Restore instrumentation and remove all test parents.
    pub(crate) fn finish(&self) {
        self.module0.call("finish", &js_sys::Array::new()).unwrap();
    }
    /// Observe native registration synchronously around standalone runtime execution.
    pub(crate) fn standalone(&self, html: &str) -> js_sys::Promise {
        self.module0
            .call("standalone", &js_sys::Array::of1(&html.into()))
            .unwrap()
            .unchecked_into()
    }
    /// Execute the wrapper with controlled ordered completion gates.
    pub(crate) fn callbacks(&self, html: &str, input: &str, expected: &str) -> js_sys::Promise {
        self.module0
            .call(
                "callbacks",
                &js_sys::Array::of3(&html.into(), &input.into(), &expected.into()),
            )
            .unwrap()
            .unchecked_into()
    }
    /// Instrument native registrations and openers below the adapter.
    pub(crate) fn begin(&self) {
        self.module1.call("begin", &js_sys::Array::new()).unwrap();
    }
    /// Start waiting before the frame is mounted or signalled.
    pub(crate) fn arm(&self, kind: &str) {
        self.module1
            .call("arm", &js_sys::Array::of1(&kind.into()))
            .unwrap();
    }
    /// Await the exact producer completion message.
    pub(crate) fn waited(&self) -> js_sys::Promise {
        self.module1
            .call("waited", &js_sys::Array::new())
            .unwrap()
            .unchecked_into()
    }
    /// Exercise native link clicks and assert routing after each completed load.
    pub(crate) fn route_check(&self, mode: &str) -> js_sys::Promise {
        self.module1
            .call("route_check", &js_sys::Array::of1(&mode.into()))
            .unwrap()
            .unchecked_into()
    }
    /// Post a controlled current-source message synchronously through native dispatch.
    pub(crate) fn dispatch(&self, kind: &str, source: &str) {
        self.module1
            .call(
                "dispatch",
                &js_sys::Array::of2(&kind.into(), &source.into()),
            )
            .unwrap();
    }
    /// Observe synchronous detach and prove closed native routes cannot deliver.
    pub(crate) fn closed(&self, expected: u32) {
        self.module1
            .call("closed", &js_sys::Array::of1(&expected.into()))
            .unwrap();
    }
    /// Return the authored load probe document.
    #[must_use]
    pub(crate) fn content(&self) -> String {
        self.module1
            .call("content", &js_sys::Array::new())
            .unwrap()
            .as_string()
            .unwrap()
    }
    /// Build a URL bootstrap with a controlled handshake.
    #[must_use]
    pub(crate) fn url(&self, mode: &str) -> String {
        self.module1
            .call("url", &js_sys::Array::of1(&mode.into()))
            .unwrap()
            .as_string()
            .unwrap()
    }
    /// Record the same consumer payload received through normal routing.
    pub(crate) fn record(&self, message: &JsValue) {
        self.module1
            .call("record", &js_sys::Array::of1(message))
            .unwrap();
    }
    /// Retain the provider's observation independently of the consumer.
    pub(crate) fn record_provider(&self, message: &JsValue) {
        self.module1
            .call("record_provider", &js_sys::Array::of1(message))
            .unwrap();
    }
    /// Configure one controlled failure below the adapter.
    pub(crate) fn fault(&self, mode: &str) {
        self.module1
            .call("fault", &js_sys::Array::of1(&mode.into()))
            .unwrap();
    }
    /// Restore instrumentation after native cleanup.
    pub(crate) fn end(&self) {
        self.module1.call("end", &js_sys::Array::new()).unwrap();
    }
    /// Dispatch native events through the injected navigation handler.
    pub(crate) fn navigation(&self, html: &str, input: &str, expected: &str) -> js_sys::Promise {
        self.module0
            .call(
                "navigation",
                &js_sys::Array::of3(&html.into(), &input.into(), &expected.into()),
            )
            .unwrap()
            .unchecked_into()
    }
    /// Observe the mounted native validation document, or accepted recovered input.
    pub(crate) fn validation_check(&self, expected: &str, css: &str) -> js_sys::Promise {
        self.module1
            .call(
                "validation_check",
                &js_sys::Array::of2(&expected.into(), &css.into()),
            )
            .unwrap()
            .unchecked_into()
    }
}
