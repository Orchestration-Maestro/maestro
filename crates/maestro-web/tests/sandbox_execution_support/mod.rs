//! Shared public execution witnesses and native effect observations.
use maestro_app::browser::components::{
    sandbox::sandbox_runtime_provider::{SandboxRespond, SandboxRuntimeProvider},
    sandboxed_iframe::{SandboxEnvironment, SandboxExecution},
};
use maestro_app::{BoxFuture, Cancellation, DiagnosticErrorInfo, SharedHandle};
use maestro_web::{SandboxIframe, browser::sandbox_value::BrowserSandboxValue};
use std::{cell::RefCell, rc::Rc, sync::Arc};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(module = "/tests/sandbox_execution_support/native.js")]
extern "C" {
    /// Install existing native subscription observations plus timer observations.
    pub fn begin();
    /// Assert release and restore every replaced native API.
    pub fn end();
    /// Select an existing controlled native failure.
    pub fn fault(mode: &str);
    /// Assert the existing native listener count.
    pub fn closed(count: u32);
    /// Observe an explicit parent-directed readiness signal.
    pub fn watch(tag: &str) -> js_sys::Promise;
    /// Invoke the captured production deadline, checking the delay.
    pub fn deadline();
    /// Assert the live production timer count.
    pub fn timers(count: u32);
    /// Native bootstrap URL, with optional automatic readiness.
    pub fn bootstrap(ready: bool) -> String;
    /// Dispatch current, unrelated or repeated native handshake messages.
    pub fn dispatch(tag: &str, current: bool);
    /// Retain the actual current native message source.
    pub fn remember_frame() -> JsValue;
    /// Dispatch from a retained, possibly detached native source.
    pub fn dispatch_source(tag: &str, source: &JsValue);
    /// Release the callback gate through the actual child window.
    pub fn release_frame();
    /// Release one selected sibling through its captured native window.
    pub fn release_source(source: &JsValue);
    /// Replace parser results for explicit validation outcomes.
    pub fn parser(mode: &str);
    /// Restore a parser after a controlled failure.
    pub fn restore_parser();
    /// Independently assert each retained native kind and value.
    pub fn assert_native(name: &str, value: &JsValue);
    /// Remove test-owned placement parents.
    pub fn finish();
}
/// Runtime provider allocation used unchanged in all witnesses.
pub type Provider = SharedHandle<dyn SandboxRuntimeProvider<BrowserSandboxValue>>;
/// Ordered call and message trace owned by each test transaction.
pub type Trace = Rc<RefCell<Vec<String>>>;
/// Native hook effect retaining its original diagnostic.
pub type Hook = Rc<dyn Fn() -> Result<(), DiagnosticErrorInfo>>;
/// Execution-start hook observing the supplied cancellation handle.
pub type Start = Rc<dyn Fn(Option<Cancellation>) -> Result<(), DiagnosticErrorInfo>>;
/// Raw message observer preserving native identity.
pub type Observe = Rc<dyn Fn(&BrowserSandboxValue)>;
/// Controlled provider with distinct callback phases.
#[derive(Default)]
pub struct Probe {
    /// Label used only in this test's ordered transcript.
    pub label: &'static str,
    /// One test-local transcript.
    pub trace: Trace,
    /// Self-contained native helper source.
    pub runtime: String,
    /// Optional start effect.
    pub start: Option<Start>,
    /// Optional terminal effect.
    pub end: Option<Hook>,
    /// Optional native message observation.
    pub observe: Option<Observe>,
    /// Preparation phase that rejects.
    pub failure: Option<&'static str>,
}
impl SandboxRuntimeProvider<BrowserSandboxValue> for Probe {
    fn get_data(
        &self,
    ) -> Result<indexmap::IndexMap<String, BrowserSandboxValue>, DiagnosticErrorInfo> {
        if self.failure == Some("data") {
            return Err(DiagnosticErrorInfo::error("data failure"));
        }
        Ok(indexmap::IndexMap::new())
    }
    fn get_runtime(&self) -> Result<String, DiagnosticErrorInfo> {
        if self.failure == Some("runtime") {
            return Err(DiagnosticErrorInfo::error("runtime failure"));
        }
        Ok(if self.runtime.is_empty() {
            "function(){}".into()
        } else {
            self.runtime.clone()
        })
    }
    fn get_description(&self) -> Result<String, DiagnosticErrorInfo> {
        Ok("controlled helper".into())
    }
    fn on_execution_start(
        &self,
        id: &str,
        signal: Option<Cancellation>,
    ) -> Result<(), DiagnosticErrorInfo> {
        self.trace
            .borrow_mut()
            .push(format!("start:{}:{id}", self.label));
        self.start.as_ref().map_or(Ok(()), |hook| hook(signal))
    }
    fn on_execution_end(&self, id: &str) -> Result<(), DiagnosticErrorInfo> {
        self.trace
            .borrow_mut()
            .push(format!("end:{}:{id}", self.label));
        self.end.as_ref().map_or(Ok(()), |hook| hook())
    }
    fn handle_message(
        &self,
        message: BrowserSandboxValue,
        respond: SandboxRespond<BrowserSandboxValue>,
    ) -> BoxFuture<Result<(), DiagnosticErrorInfo>> {
        self.trace
            .borrow_mut()
            .push(format!("provider:{}:{}", self.label, tag(&message)));
        if let Some(observe) = &self.observe {
            observe(&message);
        }
        if tag(&message) == "custom" {
            let response = BrowserSandboxValue(js_sys::JSON::parse("{\"success\":true}").unwrap());
            respond(response).unwrap();
        }
        Box::pin(async { Ok(()) })
    }
}
/// Additional consumer recording the exact message sequence.
pub struct Consumer(pub &'static str, pub Trace);
impl maestro_web::components::sandbox::runtime_message_router::MessageConsumer<BrowserSandboxValue>
    for Consumer
{
    fn handle_message(
        &self,
        message: BrowserSandboxValue,
    ) -> BoxFuture<Result<(), DiagnosticErrorInfo>> {
        self.1
            .borrow_mut()
            .push(format!("consumer:{}:{}", self.0, tag(&message)));
        Box::pin(async { Ok(()) })
    }
}
/// Read the original message tag without a JSON projection.
pub fn tag(value: &BrowserSandboxValue) -> String {
    field(value, "type").as_string().unwrap()
}
/// Read one native property.
pub fn field(value: &BrowserSandboxValue, key: &str) -> JsValue {
    js_sys::Reflect::get(&value.0, &key.into()).unwrap()
}
/// Reuse one authored identifier across sequential runs; each run has its own assertions.
pub fn request(code: &str, providers: Vec<Provider>) -> SandboxExecution<BrowserSandboxValue> {
    SandboxExecution {
        sandbox_id: "execution".into(),
        code: code.into(),
        runtime_providers: providers,
        signal: None,
    }
}
/// Poll once without spawning or introducing a test deadline.
pub fn pending<T>(future: &mut std::pin::Pin<Box<impl std::future::Future<Output = T> + ?Sized>>) {
    assert!(
        future
            .as_mut()
            .poll(&mut std::task::Context::from_waker(std::task::Waker::noop()))
            .is_pending()
    );
}
/// Run through the actual application formatting owner and hidden adapter.
pub async fn application(
    code: &str,
    providers: Vec<Provider>,
) -> Result<maestro_app::browser::tools::javascript_repl::JavaScriptExecution, DiagnosticErrorInfo>
{
    maestro_app::browser::tools::javascript_repl::execute_javascript(
        code.into(),
        providers,
        None,
        SandboxEnvironment {
            host: Arc::new(maestro_web::BrowserSandboxHost),
            sandbox_url_provider: None,
        },
    )
    .await
}
/// Downloads use the existing application runtime provider, including its acknowledgement.
pub fn downloads() -> Provider {
    SharedHandle::new(maestro_app::browser::components::sandbox::file_download_runtime_provider::FileDownloadRuntimeProvider::<BrowserSandboxValue>::new())
}
/// Assert no owned parent or native frame survives a hidden call.
pub fn hidden_released() {
    let document = web_sys::window().unwrap().document().unwrap();
    assert!(document.query_selector("sandbox-iframe").unwrap().is_none());
    assert!(document.query_selector("iframe").unwrap().is_none());
}
/// Direct retained execution returning its native payload.
pub async fn direct(
    sandbox: &SandboxIframe,
    code: &str,
    providers: Vec<Provider>,
) -> maestro_app::SandboxResult<BrowserSandboxValue, BrowserSandboxValue> {
    sandbox
        .execute(request(code, providers), vec![], false)
        .await
        .unwrap()
}

/// Only consumed authored inputs and final oracle outputs are committed.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReturnCase {
    /// Selects an independent native-kind assertion.
    pub name: String,
    /// Authored program executed through both public routes.
    pub code: String,
    /// Exact output observed from the source tool.
    pub output: String,
}
/// Read the strict return corpus.
pub fn return_cases() -> Vec<ReturnCase> {
    corpus().returns
}
/// Strict envelope of the two consumed oracle groups.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    /// Native return programs and final displays.
    returns: Vec<ReturnCase>,
    /// Default registration metadata.
    tool: ToolDefaults,
}
/// Exact default registration metadata observed through the source factory.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolDefaults {
    /// Public tool name.
    pub name: String,
    /// Public tool label.
    pub label: String,
    /// Complete public input schema.
    pub parameters: serde_json::Value,
}
/// Parse the one consumed fixture format without accepting extra fields.
fn corpus() -> Corpus {
    serde_json::from_str(include_str!("../fixtures/sandbox_execution.json")).unwrap()
}
/// Read the existing factory's registration defaults.
pub fn tool_defaults() -> ToolDefaults {
    corpus().tool
}
/// Execute the actual thread-local tool registration without holding its settings guard.
pub async fn tool_output(code: &str) -> Result<String, DiagnosticErrorInfo> {
    let execute =
        maestro_web::JAVASCRIPT_REPL_TOOL.with(|tool| tool.tool.read().unwrap().execute.clone());
    let result = execute(
        "native".into(),
        serde_json::json!({"title":"run","code":code}),
        None,
        None,
    )
    .await?;
    let maestro_app::UserBlock::Text(text) = &result.content[0] else {
        panic!("text result")
    };
    Ok(text.text.clone())
}

/// Native binary content retained by a provider as well as by the returned result.
pub type FileAlias = Rc<RefCell<Option<js_sys::Uint8Array>>>;
/// Capture original file fields, then edit the buffer and replace the message fields at end.
pub fn file_editor() -> (Provider, FileAlias) {
    use wasm_bindgen::JsCast as _;
    let message = Rc::new(RefCell::new(None::<BrowserSandboxValue>));
    let original = FileAlias::default();
    let observed_bytes = original.clone();
    let observed = message.clone();
    let hook = Probe {
        observe: Some(Rc::new(move |value| {
            if tag(value) == "file-returned"
                && field(value, "fileName").as_string().as_deref() == Some("binary")
            {
                observed.replace(Some(value.clone()));
                observed_bytes.replace(Some(
                    field(value, "content").unchecked_into::<js_sys::Uint8Array>(),
                ));
            }
        })),
        end: Some(Rc::new(move || {
            let message = message.borrow().clone().unwrap();
            let bytes = field(&message, "content").unchecked_into::<js_sys::Uint8Array>();
            bytes.set_index(0, 255);
            js_sys::Reflect::set(&message.0, &"fileName".into(), &"replacement".into()).unwrap();
            js_sys::Reflect::set(
                &message.0,
                &"content".into(),
                &js_sys::Uint8Array::from(&[8u8][..]),
            )
            .unwrap();
            Ok(())
        })),
        ..Probe::default()
    };
    (SharedHandle::new(hook), original)
}
/// Distinct first/later start failures and preparation failures share the same public route.
pub fn setup_providers(phase: &str, trace: &Trace) -> Vec<Provider> {
    let first = Probe {
        label: "first",
        trace: trace.clone(),
        start: (phase == "first-start")
            .then(|| Rc::new(|_| Err(DiagnosticErrorInfo::error("first-start failure"))) as Start),
        ..Probe::default()
    };
    let second = Probe {
        label: "second",
        trace: trace.clone(),
        start: (phase == "later-start")
            .then(|| Rc::new(|_| Err(DiagnosticErrorInfo::error("later-start failure"))) as Start),
        failure: match phase {
            "data" => Some("data"),
            "runtime" => Some("runtime"),
            _ => None,
        },
        ..Probe::default()
    };
    vec![SharedHandle::new(first), SharedHandle::new(second)]
}

/// Construct and consume the selected root records through the actual hidden host.
pub async fn root_records() {
    use maestro_app::browser::components::sandboxed_iframe::{SandboxFileContent, SandboxHost};
    use maestro_web::{
        BrowserSandboxHost, SandboxFile, SandboxResult, create_javascript_repl_tool,
    };
    let sandbox = BrowserSandboxHost.create_hidden(None).unwrap();
    let result: SandboxResult<BrowserSandboxValue> = sandbox
        .execute(request(
            "await returnDownloadableFile('root','root-text','text/plain');",
            vec![downloads()],
        ))
        .await
        .unwrap();
    assert!(result.success);
    let file: SandboxFile = result.files.unwrap().remove(0);
    assert_eq!(file.file_name, "root");
    assert_eq!(file.mime_type, "text/plain");
    assert_eq!(file.content, SandboxFileContent::Text("root-text".into()));
    sandbox.remove();
    let local = create_javascript_repl_tool(Arc::new(BrowserSandboxHost));
    let execute = local.tool.read().unwrap().execute.clone();
    let result = execute(
        "root".into(),
        serde_json::json!({"title":"root","code":"return 4;"}),
        None,
        None,
    )
    .await
    .unwrap();
    let maestro_app::UserBlock::Text(text) = &result.content[0] else {
        panic!("text result")
    };
    assert_eq!(text.text, "=> 4");
    hidden_released();
}

/// Retain the actual terminal envelope and edit it only during the end hook.
pub fn terminal_editor(kind: &'static str, script: &'static str) -> Provider {
    let message = Rc::new(RefCell::new(None::<BrowserSandboxValue>));
    let observed = message.clone();
    SharedHandle::new(Probe {
        observe: Some(Rc::new(move |value| {
            if tag(value) == kind {
                observed.replace(Some(value.clone()));
            }
        })),
        end: Some(Rc::new(move || {
            js_sys::Function::new_with_args("m", script)
                .call1(&JsValue::UNDEFINED, &message.borrow().as_ref().unwrap().0)
                .unwrap();
            Ok(())
        })),
        ..Probe::default()
    })
}
