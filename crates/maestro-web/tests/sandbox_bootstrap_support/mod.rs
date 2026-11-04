//! Prototype bootstrap documents, policies and isolated-frame observations.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::contributions_support::fetch_text;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use js_sys::{Function, Object, Promise, Reflect, Uint8Array};
use std::{cell::Cell, fmt::Write as _};
use wasm_bindgen::{JsCast as _, JsValue, closure::Closure};
use wasm_bindgen_futures::JsFuture;
use web_sys::{Document, HtmlIFrameElement, MessageEvent, Window};

/// Global the prototype's generated binding glue initializes.
pub(crate) const GLOBAL: &str = "__maestroSandboxRuntime";
/// Served prototype bundle directory.
const BUNDLE: &str = "/package/modules/sandbox_runtime/";
/// Sandbox flags of every authored-content frame.
const FLAGS: &str = "allow-scripts allow-modals";
/// Upper bound of the synchronous compilation search.
const SEARCH_LIMIT: usize = 64 << 20;
/// Records uncaught errors before the runtime and suppresses their default report.
pub(crate) const OBSERVER: &str = "window.S={errors:[]};addEventListener('error',e=>{S.errors.push(e.error?e.error.name+': '+e.error.message:'muted: '+e.message);e.preventDefault()})";

thread_local! {
    /// Distinct observation box per mounted frame.
    static SERIAL: Cell<u32> = const { Cell::new(0) };
}

/// Record a qualification measurement in the browser log.
pub(crate) fn receipt(text: &str) {
    web_sys::console::log_1(&format!("RECEIPT bootstrap {text}").into());
}

/// Generated binding glue and the matching WebAssembly bytes.
pub(crate) struct Bundle {
    /// Classic-script glue emitted by the binding generator.
    pub glue: String,
    /// Release WebAssembly module.
    pub wasm: Vec<u8>,
}

impl Bundle {
    /// Read the packaged prototype bundle from the fixture origin.
    pub(crate) async fn load() -> Self {
        let glue = fetch_text(&format!("{BUNDLE}runtime.js")).await;
        let response: web_sys::Response = JsFuture::from(
            web_sys::window()
                .unwrap()
                .fetch_with_str(&format!("{BUNDLE}runtime_bg.wasm")),
        )
        .await
        .unwrap()
        .dyn_into()
        .unwrap();
        assert!(response.ok(), "runtime bundle HTTP {}", response.status());
        let bytes = JsFuture::from(response.array_buffer().unwrap())
            .await
            .unwrap();
        let wasm = Uint8Array::new(&bytes).to_vec();
        Self { glue, wasm }
    }

    /// Glue and synchronous loader in one inline script, for documents with a hierarchical URL.
    pub(crate) fn inline(&self, wasm: &[u8]) -> Boot {
        Boot {
            prefix: String::new(),
            script: format!("{}\n{}", self.glue, loader(wasm)),
        }
    }

    /// Served glue, then the inline synchronous loader, for `about:srcdoc` documents.
    pub(crate) fn served(wasm: &[u8]) -> Boot {
        Boot {
            prefix: SERVED.into(),
            script: loader(wasm),
        }
    }

    /// Served glue, then an inline asynchronous loader, for the staging alternative.
    pub(crate) fn staged(&self) -> Boot {
        Boot {
            prefix: SERVED.into(),
            script: format!(
                "{GLOBAL}({{module_or_path:Uint8Array.fromBase64(\"{}\")}});",
                STANDARD.encode(&self.wasm)
            ),
        }
    }
}

/// Glue element that loads from the fixture origin, parser-blocking, with unmuted errors.
const SERVED: &str =
    "<script src=\"/package/modules/sandbox_runtime/runtime.js\" crossorigin></script>";

/// Inline statement that instantiates the embedded module synchronously.
fn loader(wasm: &[u8]) -> String {
    format!(
        "{GLOBAL}.initSync({{module:Uint8Array.fromBase64(\"{}\")}});",
        STANDARD.encode(wasm)
    )
}

/// Markup that precedes the configuration block and the inline loading script text.
pub(crate) struct Boot {
    /// Elements placed before the carriers, such as a served glue script.
    pub prefix: String,
    /// Inline loading script text.
    pub script: String,
}

/// One prototype runtime insertion and the exact script texts a hash policy admits.
pub(crate) struct Runtime {
    /// Inert carriers, configuration and loading script.
    pub markup: String,
    /// Loading script text.
    pub boot: String,
    /// Decoded provider invocation texts in carrier order.
    pub providers: Vec<String>,
}

impl Runtime {
    /// Assemble the boot prefix and carriers before the configuration block, then the loader.
    pub(crate) fn new(
        boot: Boot,
        config: &serde_json::Value,
        id: &str,
        providers: &[&str],
    ) -> Self {
        let quoted = serde_json::Value::String(id.into());
        let providers: Vec<String> = providers
            .iter()
            .map(|source| format!("({source})({quoted});"))
            .collect();
        let Boot {
            prefix: mut markup,
            script: boot,
        } = boot;
        // Writing into a `String` cannot fail.
        for provider in &providers {
            let _ = write!(
                markup,
                "<template data-maestro-sandbox-provider><svg><script>{}</script></svg></template>",
                encode(provider)
            );
        }
        let _ = write!(
            markup,
            "<script type=\"application/json\" data-maestro-sandbox>{}</script><script>{boot}</script>",
            config.to_string().replace('<', "\\u003c")
        );
        Self {
            markup,
            boot,
            providers,
        }
    }
}

/// Authored document carrying the `head` script, the loader with one attachment selection, then
/// the `body` script.
pub(crate) fn loader_document(head: &str, boot: Boot, body: &str) -> String {
    let id = "bootstrap-loader";
    let config =
        serde_json::json!({"sandboxId":id,"globals":{"attachments":[]},"runtime":["attachments"]});
    format!(
        "<html><head><script>{head}</script>{}</head><body><script>{body}</script></body></html>",
        Runtime::new(boot, &config, id, &[]).markup
    )
}

/// Encode text for an SVG script carrier.
pub(crate) fn encode(code: &str) -> String {
    code.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('\r', "&#13;")
}

/// Asynchronous program text settled by the prototype's `run`.
pub(crate) fn program(code: &str) -> String {
    format!("{GLOBAL}.run(async () => {{\n{code}\n}});")
}

/// Wrap a program module after the runtime in a complete document.
pub(crate) fn wrap(runtime: &str, program: &str) -> String {
    format!(
        "<!DOCTYPE html>\n<html>\n<head>\n{runtime}\n</head>\n<body>\n<svg display=\"none\" data-maestro-sandbox-program><script type=\"module\">{}</script></svg>\n</body>\n</html>",
        encode(program)
    )
}

/// Pad a module with one custom section to exactly `size` bytes; its own size is returned unchanged.
pub(crate) fn padded(wasm: &[u8], size: usize) -> Vec<u8> {
    const NAME: &[u8] = b"maestro-padding";
    if size == wasm.len() {
        return wasm.to_vec();
    }
    let content = (1..=5)
        .map(|width| size - wasm.len() - 1 - width)
        .find(|content| leb(*content).len() == size - wasm.len() - 1 - content)
        .unwrap();
    let mut bytes = wasm.to_vec();
    bytes.push(0);
    bytes.extend(leb(content));
    bytes.extend(leb(NAME.len()));
    bytes.extend(NAME);
    bytes.resize(size, 0);
    assert_eq!(bytes.len(), size);
    bytes
}

/// Unsigned LEB128 encoding.
fn leb(mut value: usize) -> Vec<u8> {
    let mut bytes = Vec::new();
    loop {
        let byte = u8::try_from(value & 0x7f).unwrap();
        value >>= 7;
        if value == 0 {
            bytes.push(byte);
            return bytes;
        }
        bytes.push(byte | 0x80);
    }
}

/// Compile synchronously on this main thread, returning the refusal text.
pub(crate) fn compile(wasm: &[u8]) -> Result<(), String> {
    js_sys::WebAssembly::Module::new(&Uint8Array::from(wasm).into())
        .map(drop)
        .map_err(|error| {
            let error: js_sys::Error = error.unchecked_into();
            format!(
                "{}: {}",
                String::from(error.name()),
                String::from(error.message())
            )
        })
}

/// Largest padded size that compiles synchronously and the first refused size with its error.
pub(crate) fn synchronous_limit(wasm: &[u8]) -> (usize, Option<(usize, String)>) {
    compile(wasm).unwrap();
    let Err(error) = compile(&padded(wasm, SEARCH_LIMIT)) else {
        return (SEARCH_LIMIT, None);
    };
    let (mut passing, mut refused, mut message) = (wasm.len(), SEARCH_LIMIT, error);
    while refused - passing > 1 {
        let middle = passing + (refused - passing) / 2;
        match compile(&padded(wasm, middle)) {
            Ok(()) => passing = middle,
            Err(error) => (refused, message) = (middle, error),
        }
    }
    (passing, Some((refused, message)))
}

/// Quoted CSP source admitting one exact script text.
pub(crate) async fn digest(text: &str) -> String {
    let subtle = Reflect::get(
        &Reflect::get(&js_sys::global(), &"crypto".into()).unwrap(),
        &"subtle".into(),
    )
    .unwrap();
    let operation: Function = Reflect::get(&subtle, &"digest".into())
        .unwrap()
        .dyn_into()
        .unwrap();
    let hashed = JsFuture::from(Promise::from(
        operation
            .call2(
                &subtle,
                &"SHA-256".into(),
                &Uint8Array::from(text.as_bytes()),
            )
            .unwrap(),
    ))
    .await
    .unwrap();
    format!(
        "'sha256-{}'",
        STANDARD.encode(Uint8Array::new(&hashed).to_vec())
    )
}

/// Hash sources for every supplied text, space separated.
pub(crate) async fn digests(texts: &[&str]) -> String {
    let mut sources = Vec::new();
    for text in texts {
        sources.push(digest(text).await);
    }
    sources.join(" ")
}

/// How a probe frame receives its document and its inherited policy.
#[derive(Default)]
pub(crate) struct Mount<'a> {
    /// Policy the same-origin host document declares before the frame exists.
    pub policy: &'a str,
    /// Install a default Trusted Types policy that only creates HTML.
    pub trusted_default: bool,
    /// Load the supplied sandbox URL and post the document instead of srcdoc.
    pub url: bool,
    /// Wait for the staged runtime's message instead of the frame's load.
    pub staged: bool,
}

/// One mounted probe frame inside its policy host.
pub(crate) struct Mounted {
    /// Same-origin host frame carrying the inherited policy.
    pub host: HtmlIFrameElement,
    /// Observation box identifier read by the driver.
    pub id: String,
    /// Payload of the staged runtime's completion message, when awaited.
    pub staged: JsValue,
}

/// Mount, read and remove one probe frame, returning its state and staged message.
pub(crate) async fn observe(html: &str, options: Mount<'_>) -> (serde_json::Value, JsValue) {
    let mounted = mount(html, options).await;
    let state = mounted.inspect().await;
    mounted.remove();
    (state, mounted.staged)
}

impl Mounted {
    /// Read the frame's retained probe state through the driver.
    pub(crate) async fn inspect(&self) -> serde_json::Value {
        let inspect: Function = Reflect::get(&js_sys::global(), &"inspectSandbox".into())
            .unwrap()
            .dyn_into()
            .unwrap();
        let value = JsFuture::from(Promise::from(
            inspect
                .call2(&JsValue::NULL, &self.id.clone().into(), &"state".into())
                .unwrap(),
        ))
        .await
        .unwrap();
        json(&value)
    }

    /// Remove the host and everything it contains.
    pub(crate) fn remove(&self) {
        self.host.remove();
    }
}

/// Convert a browser value through native JSON.
pub(crate) fn json(value: &JsValue) -> serde_json::Value {
    let text = js_sys::JSON::stringify(value).unwrap();
    serde_json::from_str(&String::from(text)).unwrap()
}

/// Resolve on the element's next load event.
fn loaded(element: &web_sys::HtmlElement) -> JsFuture {
    JsFuture::from(Promise::new(&mut |resolve, _| {
        element.set_onload(Some(&resolve));
    }))
}

/// Resolve with the first message of `kind` whose source is the frame's current window.
fn message(
    window: &Window,
    frame: &HtmlIFrameElement,
    kind: &str,
    reply: Option<&JsValue>,
) -> JsFuture {
    let (frame, kind, reply) = (frame.clone(), kind.to_owned(), reply.cloned());
    let window = window.clone();
    JsFuture::from(Promise::new(&mut |resolve, _| {
        let (frame, kind, reply, target) =
            (frame.clone(), kind.clone(), reply.clone(), window.clone());
        let listener = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
            let source = frame.content_window();
            let current = source
                .as_ref()
                .is_some_and(|source| Object::is(&event.source().into(), source));
            let data = event.data();
            let matches = data.is_object()
                && Reflect::get(&data, &"type".into()).unwrap().as_string() == Some(kind.clone());
            if !(current && matches) {
                return;
            }
            if let (Some(reply), Some(source)) = (&reply, source) {
                source.post_message(reply, "*").unwrap();
            } else {
                resolve.call1(&JsValue::UNDEFINED, &data).unwrap();
            }
        });
        target
            .add_event_listener_with_callback("message", listener.as_ref().unchecked_ref())
            .unwrap();
        // The harness never removes this listener; it lives as long as the test page.
        listener.forget();
    }))
}

/// Create a same-origin host that declares the policy before any frame exists.
async fn host(options: &Mount<'_>) -> (HtmlIFrameElement, Document, Window) {
    let document = web_sys::window().unwrap().document().unwrap();
    let host: HtmlIFrameElement = document
        .create_element("iframe")
        .unwrap()
        .dyn_into()
        .unwrap();
    let ready = loaded(&host);
    document.body().unwrap().append_child(&host).unwrap();
    ready.await.unwrap();
    let inner: Document = host.content_document().unwrap();
    let window = host.content_window().unwrap();
    if options.trusted_default {
        trusted_default(&window);
    }
    if !options.policy.is_empty() {
        let meta = inner.create_element("meta").unwrap();
        meta.set_attribute("http-equiv", "Content-Security-Policy")
            .unwrap();
        meta.set_attribute("content", options.policy).unwrap();
        inner
            .query_selector("head")
            .unwrap()
            .unwrap()
            .append_child(&meta)
            .unwrap();
    }
    (host, inner, window)
}

/// Mount the sandboxed probe frame inside a policy host and await its document.
pub(crate) async fn mount(html: &str, options: Mount<'_>) -> Mounted {
    let (host, inner, window) = host(&options).await;
    let id = SERIAL.with(|serial| {
        serial.set(serial.get() + 1);
        format!("bootstrap-probe-{}", serial.get())
    });
    let container = inner.create_element("div").unwrap();
    container.set_id(&id);
    inner.body().unwrap().append_child(&container).unwrap();
    // The host document belongs to another realm, so its frame fails a native type check.
    let frame: HtmlIFrameElement = inner.create_element("iframe").unwrap().unchecked_into();
    frame.set_attribute("sandbox", FLAGS).unwrap();
    let mut staged = JsValue::UNDEFINED;
    if options.url {
        let reply = js_sys::JSON::parse(
            &serde_json::json!({"type":"sandbox-load","code":html}).to_string(),
        )
        .unwrap();
        drop(message(&window, &frame, "sandbox-ready", Some(&reply)));
        let installed = message(&window, &frame, "sandbox-installed", None);
        frame.set_src(&format!(
            "{}/sandbox.html",
            web_sys::window().unwrap().location().origin().unwrap()
        ));
        container.append_child(&frame).unwrap();
        installed.await.unwrap();
    } else if options.staged {
        let done = message(&window, &frame, "sandbox-staged", None);
        frame.set_srcdoc(html);
        container.append_child(&frame).unwrap();
        staged = done.await.unwrap();
    } else {
        let done = loaded(&frame);
        frame.set_srcdoc(html);
        container.append_child(&frame).unwrap();
        done.await.unwrap();
    }
    Mounted { host, id, staged }
}

/// Install a host default policy that creates HTML only, as embedding pages may.
fn trusted_default(window: &Window) {
    let types = Reflect::get(window, &"trustedTypes".into()).unwrap();
    let rules = Object::new();
    let identity = Closure::<dyn Fn(JsValue) -> JsValue>::new(|text| text).into_js_value();
    Reflect::set(&rules, &"createHTML".into(), &identity).unwrap();
    let create: Function = Reflect::get(&types, &"createPolicy".into())
        .unwrap()
        .dyn_into()
        .unwrap();
    create.call2(&types, &"default".into(), &rules).unwrap();
}
