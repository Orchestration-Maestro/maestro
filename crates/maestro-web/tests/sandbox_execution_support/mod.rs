//! Shared public execution witnesses and native effect observations.
use crate::browser_fixture_support::{Patch, dom_exception, error, get, global, invoke, object};
use maestro_app::Runtime;
use maestro_app::browser::components::{
    sandbox::providers::{SandboxRespond, SandboxRuntimeProvider},
    sandboxed_iframe::{SandboxEnvironment, SandboxExecution},
};
use maestro_models::{BoxFuture, Cancellation, DiagnosticErrorInfo, SharedHandle};
use maestro_web::{SandboxIframe, browser::values::BrowserSandboxValue};
use std::{cell::RefCell, rc::Rc, sync::Arc};
use wasm_bindgen::{JsCast, prelude::*};

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
    fn get_runtime(&self) -> Result<Runtime, DiagnosticErrorInfo> {
        if self.failure == Some("runtime") {
            return Err(DiagnosticErrorInfo::error("runtime failure"));
        }
        Ok(Runtime::Content(if self.runtime.is_empty() {
            "function(){}".into()
        } else {
            self.runtime.clone()
        }))
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
#[must_use]
pub(crate) fn tag(value: &BrowserSandboxValue) -> String {
    field(value, "type").as_string().unwrap()
}
/// Read one native property.
#[must_use]
pub(crate) fn field(value: &BrowserSandboxValue, key: &str) -> JsValue {
    js_sys::Reflect::get(&value.0, &key.into()).unwrap()
}
/// Reuse one authored identifier across sequential runs; each run has its own assertions.
#[must_use]
pub(crate) fn request(
    code: &str,
    providers: Vec<Provider>,
) -> SandboxExecution<BrowserSandboxValue> {
    SandboxExecution {
        sandbox_id: "execution".into(),
        code: code.into(),
        runtime_providers: providers,
        signal: None,
    }
}
/// Poll once without spawning or introducing a test deadline.
pub(crate) fn pending<T>(
    future: &mut std::pin::Pin<Box<impl std::future::Future<Output = T> + ?Sized>>,
) {
    assert!(
        future
            .as_mut()
            .poll(&mut std::task::Context::from_waker(std::task::Waker::noop()))
            .is_pending()
    );
}
/// Run through the actual application formatting owner and hidden adapter.
pub(crate) async fn application(
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
#[must_use]
pub(crate) fn downloads() -> Provider {
    SharedHandle::new(maestro_app::browser::components::sandbox::file_download_runtime_provider::FileDownloadRuntimeProvider::<BrowserSandboxValue>::new())
}
/// Assert no owned parent or native frame survives a hidden call.
pub(crate) fn hidden_released() {
    let document = web_sys::window().unwrap().document().unwrap();
    assert!(document.query_selector("sandbox-iframe").unwrap().is_none());
    assert!(document.query_selector("iframe").unwrap().is_none());
}
/// Direct retained execution returning its native payload.
pub(crate) async fn direct(
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
#[must_use]
pub(crate) fn return_cases() -> Vec<ReturnCase> {
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
#[must_use]
pub(crate) fn tool_defaults() -> ToolDefaults {
    corpus().tool
}
/// Execute the actual thread-local tool registration without holding its settings guard.
pub(crate) async fn tool_output(code: &str) -> Result<String, DiagnosticErrorInfo> {
    let execute =
        maestro_web::JAVASCRIPT_REPL_TOOL.with(|tool| tool.tool.read().unwrap().execute.clone());
    let result = execute(
        "native".into(),
        serde_json::json!({"title":"run","code":code}),
        None,
        None,
    )
    .await?;
    let maestro_models::UserBlock::Text(text) = &result.content[0] else {
        panic!("text result")
    };
    Ok(text.text.clone())
}

/// Native binary content retained by a provider as well as by the returned result.
pub type FileAlias = Rc<RefCell<Option<js_sys::Uint8Array>>>;
/// Capture original file fields, then edit the buffer and replace the message fields at end.
#[must_use]
pub(crate) fn file_editor() -> (Provider, FileAlias) {
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
pub(crate) fn setup_providers(phase: &str, trace: &Trace) -> Vec<Provider> {
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
pub(crate) async fn root_records() {
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
    let maestro_models::UserBlock::Text(text) = &result.content[0] else {
        panic!("text result")
    };
    assert_eq!(text.text, "=> 4");
    hidden_released();
}

/// Retain the actual terminal envelope and edit it only during the end hook.
#[must_use]
pub(crate) fn terminal_editor(kind: &'static str, script: &'static str) -> Provider {
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

/// The live production timers and the captured two-minute deadline.
#[derive(Default)]
struct Timers {
    /// Deadline timers not yet cleared.
    active: Vec<JsValue>,
    /// The captured deadline callback and its delay.
    captured: Option<(js_sys::Function, JsValue)>,
}

/// A running execution transaction above the routing one.
struct Transaction {
    /// Live production deadlines.
    timers: Rc<RefCell<Timers>>,
    /// The replaced scheduling and clearing.
    patches: [Patch; 2],
}

/// One execution transaction at a time: routing observations plus production deadlines.
#[derive(Clone, Default)]
pub struct Fixture {
    /// The routing transaction below the adapter.
    routes: crate::sandbox_loading_support::routes::Routes,
    /// The running transaction, if any.
    active: Rc<RefCell<Option<Transaction>>>,
    /// The frame attribute fault, while selected.
    attribute: Rc<RefCell<Option<Patch>>>,
    /// The parser fault, while selected.
    parser: Rc<RefCell<Option<Patch>>>,
}

/// The two-minute production deadline.
const DEADLINE: u32 = 120_000;

impl Fixture {
    /// Retain the fixture in a live URL callback.
    #[must_use]
    pub(crate) fn bootstrap_callback(
        ready: bool,
    ) -> maestro_app::browser::components::sandboxed_iframe::SandboxUrlProvider {
        SharedHandle::new(move || Ok(Self::bootstrap(ready)))
    }
    /// Install routing observations plus deadline timer observations.
    pub(crate) fn begin(&self) {
        self.routes.begin();
        let timers: Rc<RefCell<Timers>> = Rc::default();
        let window = global("window");
        let scheduled = Rc::clone(&timers);
        let schedule = Patch::method(&window, "setTimeout", move |original, this, args| {
            let id = original.apply(&this, &args)?;
            if args.get(1).as_f64() == Some(f64::from(DEADLINE)) {
                let mut timers = scheduled.borrow_mut();
                timers.active.push(id.clone());
                timers.captured = Some((args.get(0).unchecked_into(), args.get(1)));
            }
            Ok(id)
        });
        let cleared = Rc::clone(&timers);
        let clear = Patch::method(&window, "clearInterval", move |original, this, args| {
            cleared.borrow_mut().active.retain(|id| *id != args.get(0));
            original.apply(&this, &args)
        });
        *self.active.borrow_mut() = Some(Transaction {
            timers,
            patches: [schedule, clear],
        });
    }
    /// Assert release and restore every replaced native API.
    pub(crate) fn end(&self) {
        self.attribute.borrow_mut().take();
        self.timers(0);
        let transaction = self.active.borrow_mut().take().unwrap();
        drop(transaction.patches);
        self.routes.end();
        crate::sandbox_loading_support::native::finish();
    }
    /// Select a controlled native failure; `attribute` refuses frame attributes.
    pub(crate) fn fault(&self, mode: &str) {
        self.attribute.borrow_mut().take();
        self.routes.fault(mode);
        if mode == "attribute" {
            *self.attribute.borrow_mut() = Some(attribute_refusal());
        }
    }
    /// Assert the existing native listener count.
    pub(crate) fn closed(&self, count: u32) {
        self.routes.closed(count);
    }
    /// Observe an explicit parent-directed readiness signal, listening through the page's
    /// current registration.
    pub(crate) fn watch(tag: &str) -> js_sys::Promise {
        let window = global("window");
        let add: js_sys::Function = get(&window, "addEventListener").unchecked_into();
        let remove: js_sys::Function = get(&window, "removeEventListener").unchecked_into();
        crate::sandbox_loading_support::routes::once_message(&add, &remove, tag)
    }
    /// Invoke the captured production deadline, checking the delay.
    pub(crate) fn deadline(&self) {
        let active = self.active.borrow();
        let captured = active.as_ref().unwrap().timers.borrow().captured.clone();
        let (callback, delay) = captured.unwrap();
        drop(active);
        crate::sandbox_loading_support::native::equal(
            &delay,
            &DEADLINE.into(),
            "two-minute production callback",
        );
        callback.call0(&JsValue::UNDEFINED).unwrap();
    }
    /// Assert the live production timer count.
    pub(crate) fn timers(&self, count: u32) {
        let active = self.active.borrow();
        let live = active.as_ref().unwrap().timers.borrow().active.len();
        assert_eq!(live, count as usize, "production timer set");
    }
    /// Native bootstrap URL, with optional automatic readiness.
    #[must_use]
    pub(crate) fn bootstrap(ready: bool) -> String {
        let signal = if ready {
            "sandbox-ready"
        } else {
            "bootstrap-waiting"
        };
        let document = Self::bootstrap_document(signal);
        format!(
            "data:text/html,{}",
            String::from(js_sys::encode_uri_component(&document))
        )
    }
    /// Bootstrap document that announces itself to its parent with `signal`.
    #[must_use]
    pub(crate) fn bootstrap_document(signal: &str) -> String {
        BOOTSTRAP.replace("__TYPE__", &serde_json::Value::from(signal).to_string())
    }
    /// Dispatch a handshake message carrying the bootstrap diagnostic, from the current frame
    /// or else from the page itself.
    pub(crate) fn dispatch(tag: &str, current: bool) {
        let source = if current {
            Self::remember_frame()
        } else {
            global("window")
        };
        message(tag, "bootstrap problem", "native stack", &source);
    }
    /// Retain the actual current native message source.
    #[must_use]
    pub(crate) fn remember_frame() -> JsValue {
        frame().content_window().unwrap().into()
    }
    /// Dispatch from a retained, possibly detached native source.
    pub(crate) fn dispatch_source(tag: &str, source: &JsValue) {
        message(tag, "stale", "stale", source);
    }
    /// Release the callback gate through the actual child window.
    pub(crate) fn release_frame() {
        Self::release_source(&Self::remember_frame());
    }
    /// Release one selected sibling through its captured native window.
    pub(crate) fn release_source(source: &JsValue) {
        invoke(source, "postMessage", &[&"release".into(), &"*".into()]).unwrap();
    }
    /// Replace parser results for explicit validation outcomes.
    pub(crate) fn parser(&self, mode: &str) {
        let mode = mode.to_owned();
        let prototype = get(&global("DOMParser"), "prototype");
        let patch = Patch::method(
            &prototype,
            "parseFromString",
            move |original, this, args| match mode.as_str() {
                "throw" => Err(error("controlled parse failure")),
                "empty-throw" => Err(js_sys::Object::new().into()),
                _ => parser_error(&original.apply(&this, &args)?, mode == "empty"),
            },
        );
        *self.parser.borrow_mut() = Some(patch);
    }
    /// Restore a parser after a controlled failure.
    pub(crate) fn restore_parser(&self) {
        self.parser.borrow_mut().take();
    }
    /// Independently assert each retained native kind and value.
    pub(crate) fn assert_native(name: &str, value: &JsValue) {
        native_value(name, value);
    }
}

/// The URL bootstrap: it writes the code of a load message only when its parent sent it, and
/// announces itself with the `__TYPE__` message type.
const BOOTSTRAP: &str = r"<script>addEventListener('message',e=>{if(e.source===parent&&e.data.type==='sandbox-load'){document.open();document.write(e.data.code);document.close();}});parent.postMessage({type:__TYPE__},'*');</script>";

/// Refuse setting any frame attribute.
fn attribute_refusal() -> Patch {
    let prototype = get(&global("Element"), "prototype");
    Patch::method(&prototype, "setAttribute", |original, this, args| {
        if get(&this, "localName") == "iframe" {
            return Err(dom_exception("controlled attribute", "InvalidStateError"));
        }
        original.apply(&this, &args)
    })
}

/// Append a parser error, empty or not, to a parsed document.
fn parser_error(parsed: &JsValue, empty: bool) -> Result<JsValue, JsValue> {
    let document: &web_sys::Document = parsed.unchecked_ref();
    let node = document.create_element("parsererror")?;
    node.set_text_content(Some(if empty { "" } else { "controlled parser error" }));
    document.body().unwrap().append_child(&node)?;
    Ok(parsed.clone())
}

/// Dispatch a page handshake message of `tag` from `source`.
fn message(tag: &str, error: &str, stack: &str, source: &JsValue) {
    let data = object(&[
        ("type", &tag.into()),
        ("error", &error.into()),
        ("stack", &stack.into()),
    ]);
    crate::sandbox_loading_support::routes::dispatch_message(&data, source);
}

/// The current test frame.
fn current_frame() -> Option<web_sys::HtmlIFrameElement> {
    crate::contributions_support::document()
        .query_selector("[data-sandbox-test] iframe")
        .unwrap()
        .map(JsCast::unchecked_into::<web_sys::HtmlIFrameElement>)
}

/// The current test frame, which must be mounted.
fn frame() -> web_sys::HtmlIFrameElement {
    current_frame().unwrap()
}

/// Assert one retained native kind and value by case name.
fn native_value(name: &str, value: &JsValue) {
    let json =
        |expected: &str| crate::sandbox_loading_support::native::equal_json(value, expected, name);
    match name {
        "native_null" => assert!(value.is_null(), "{name}"),
        "native_false" => json("false"),
        "native_true" => json("true"),
        "native_zero" => assert!(js_sys::Object::is(value, &0.into()), "{name}"),
        "native_negative_zero" => assert!(js_sys::Object::is(value, &(-0.0).into()), "{name}"),
        "native_small" => json("1e-7"),
        "native_large" => json("1e21"),
        "native_empty_string" => json(r#""""#),
        "native_unicode" => json("\"雪\u{FEFF}\u{0085}\""),
        "native_object" => {
            let keys = js_sys::Object::keys(value.unchecked_ref::<js_sys::Object>());
            crate::sandbox_loading_support::native::equal_json(
                &keys,
                r#"["2","10","z","a"]"#,
                name,
            );
            crate::sandbox_loading_support::native::equal_json(
                &get(value, "a"),
                "[true,null]",
                name,
            );
        }
        "native_array" => {
            assert!(js_sys::Array::is_array(value), "{name}");
            json(r#"[2,null,"x"]"#);
        }
        "native_date" => {
            assert!(value.is_instance_of::<js_sys::Date>(), "{name}");
            let iso = value.unchecked_ref::<js_sys::Date>().to_iso_string();
            assert_eq!(String::from(iso), "2020-01-02T03:04:05.000Z", "{name}");
        }
        "native_map" | "native_set" | "native_bytes" => collection(name, value),
        "native_bigint" => assert!(
            js_sys::Object::is(value, &js_sys::BigInt::from(9).into()),
            "{name}"
        ),
        "native_nan" => assert!(value.as_f64().is_some_and(f64::is_nan), "{name}"),
        "native_infinity" => assert_eq!(value.as_f64(), Some(f64::INFINITY), "{name}"),
        "native_negative_infinity" => assert_eq!(value.as_f64(), Some(f64::NEG_INFINITY), "{name}"),
        other => panic!("unconsumed native assertion {other}"),
    }
}

/// A retained map, set or byte array keeps its class and its entries in order.
fn collection(name: &str, value: &JsValue) {
    let (class, expected) = match name {
        "native_map" => (value.is_instance_of::<js_sys::Map>(), r#"[["a",1]]"#),
        "native_set" => (value.is_instance_of::<js_sys::Set>(), "[2,1]"),
        _ => (value.is_instance_of::<js_sys::Uint8Array>(), "[0,128,255]"),
    };
    assert!(class, "{name}");
    let entries: js_sys::Array = js_sys::try_iter(value)
        .unwrap()
        .unwrap()
        .map(Result::unwrap)
        .collect();
    crate::sandbox_loading_support::native::equal_json(&entries, expected, name);
}
