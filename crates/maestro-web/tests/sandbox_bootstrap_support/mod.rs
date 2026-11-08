//! Bootstrap documents, policies and isolated-frame observations for the generated runtime.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::browser_fixture_support::{construct, object, post_from};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use js_sys::{Function, Object, Promise, Reflect, Uint8Array};
use maestro_web::components::sandbox::runtime_bundle::BUNDLE;
use maestro_web::components::sandboxed_iframe::lowering::{self, Compiled};
use std::{cell::Cell, fmt::Write as _};
use wasm_bindgen::{JsCast as _, JsValue, closure::Closure};
use wasm_bindgen_futures::JsFuture;
use web_sys::{Document, HtmlIFrameElement, MessageEvent, Window};

pub(crate) mod cases;

/// Sandbox flags of every authored-content frame.
const FLAGS: &str = "allow-scripts allow-modals";
/// Records uncaught errors before the runtime and suppresses their default report.
pub(crate) const OBSERVER: &str = "window.S={errors:[]};addEventListener('error',e=>{S.errors.push(e.error?e.error.name+': '+e.error.message:'muted: '+e.message);e.preventDefault()});addEventListener('unhandledrejection',e=>{S.errors.push(e.reason?e.reason.name+': '+e.reason.message:'muted: rejection');e.preventDefault()})";

thread_local! {
    /// Distinct observation box per mounted frame.
    static SERIAL: Cell<u32> = const { Cell::new(0) };
}

/// Record a qualification measurement in the browser log.
pub(crate) fn receipt(text: &str) {
    web_sys::console::log_1(&format!("RECEIPT bootstrap {text}").into());
}

/// The generated runtime script every document carries.
pub(crate) struct Bundle {
    /// Classic-script text, exactly as the library embeds it.
    pub script: &'static str,
}

impl Bundle {
    /// The embedded bundle.
    pub(crate) fn load() -> Self {
        Self { script: BUNDLE }
    }

    /// The bundle as the inline script every path uses, srcdoc or URL.
    pub(crate) fn boot(&self) -> Boot {
        Boot {
            prefix: String::new(),
            script: self.script.to_owned(),
        }
    }
}

/// Markup that precedes the configuration block and the inline runtime script text.
pub(crate) struct Boot {
    /// Elements placed before the carriers.
    pub prefix: String,
    /// Inline runtime script text.
    pub script: String,
}

/// One runtime insertion and the exact script texts a hash policy admits.
pub(crate) struct Runtime {
    /// Inert carriers, configuration and runtime script.
    pub markup: String,
    /// Runtime script text.
    pub boot: String,
    /// Decoded provider invocation texts in carrier order.
    pub providers: Vec<String>,
}

impl Runtime {
    /// Assemble the carriers before the configuration block, then the runtime script.
    ///
    /// `config` names the sandbox identifier, an object of globals, the ordered selections and,
    /// optionally, the program capture property; the document is a standalone one.
    pub(crate) fn new(
        boot: Boot,
        config: &serde_json::Value,
        id: &str,
        providers: &[&str],
    ) -> Self {
        let mut production = serde_json::json!({
            "sandboxId": id,
            "globals": config["globals"]
                .as_object()
                .map(|globals| globals.iter().map(|(key, value)| serde_json::json!([key, value])).collect::<Vec<_>>())
                .unwrap_or_default(),
            "runtime": config["runtime"],
            "standalone": true,
            "completion": maestro_web::components::sandboxed_iframe::COMPLETION_PROPERTY,
        });
        if let Some(capture) = config.get("programCapture") {
            production["program"] = capture.clone();
        }
        let providers: Vec<Compiled> = providers
            .iter()
            .map(|source| lowering::provider(source).unwrap())
            .collect();
        let Boot {
            prefix: mut markup,
            script: boot,
        } = boot;
        // Writing into a `String` cannot fail.
        for provider in &providers {
            let _ = write!(
                markup,
                "<template data-maestro-sandbox-provider data-maestro-capture=\"{}\"><svg><script>{}</script></svg></template>",
                provider.capture(),
                encode(provider.code())
            );
        }
        let _ = write!(
            markup,
            "<script type=\"application/json\" data-maestro-sandbox>{}</script><script>{boot}</script>",
            production.to_string().replace('<', "\\u003c")
        );
        Self {
            markup,
            boot,
            providers: providers
                .into_iter()
                .map(|provider| provider.code().to_owned())
                .collect(),
        }
    }
}

/// Authored document with a `head` script and a `body` script; the host inserts the runtime.
pub(crate) fn loader_document(head: &str, body: &str) -> String {
    format!("<html><head><script>{head}</script></head><body><script>{body}</script></body></html>")
}

/// Encode text for an SVG script carrier.
pub(crate) fn encode(code: &str) -> String {
    code.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('\r', "&#13;")
}

/// Supplied asynchronous body compiled for the prototype's Rust outcome observer.
pub(crate) fn program(code: &str) -> Compiled {
    lowering::program(code).unwrap()
}

/// Wrap a program module after the runtime in a complete document.
pub(crate) fn wrap(runtime: &str, program: &str) -> String {
    format!(
        "<!DOCTYPE html>\n<html>\n<head>\n{runtime}\n</head>\n<body>\n<svg display=\"none\" data-maestro-sandbox-program><script type=\"module\">{}</script></svg>\n</body>\n</html>",
        encode(program)
    )
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

/// Mount a bootstrap at the page origin, where its `attribute` is `value`, beside a sibling
/// frame; once it sends `signal`, check it ignores a load from the sibling and writes one
/// from its parent.
pub(crate) async fn loads_only_from_parent(attribute: &str, value: &str, signal: &str) {
    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();
    let body = document.body().unwrap();
    let sibling: HtmlIFrameElement = document.create_element("iframe").unwrap().unchecked_into();
    body.append_child(&sibling).unwrap();
    let frame: HtmlIFrameElement = document.create_element("iframe").unwrap().unchecked_into();
    frame.set_attribute(attribute, value).unwrap();
    let announced = message(&window, &frame, signal, None);
    body.append_child(&frame).unwrap();
    announced.await.unwrap();
    let foreign = sibling.content_window().unwrap();
    assert!(!writes_load(&frame, &foreign), "load from a sibling window");
    assert!(writes_load(&frame, &window), "load from the parent window");
    frame.remove();
    sibling.remove();
}

/// Deliver a load message from `source` to the frame and report whether it wrote the code.
fn writes_load(frame: &HtmlIFrameElement, source: &Window) -> bool {
    let data =
        js_sys::JSON::parse(r#"{"type":"sandbox-load","code":"<p id=\"written\"></p>"}"#).unwrap();
    let event = construct(
        "MessageEvent",
        &[
            &"message".into(),
            &object(&[("data", &data), ("source", source.as_ref())]),
        ],
    );
    frame
        .content_window()
        .unwrap()
        .dispatch_event(event.unchecked_ref())
        .unwrap();
    frame
        .content_document()
        .unwrap()
        .get_element_by_id("written")
        .is_some()
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
        let (frame, kind, reply, host) =
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
                // A real host replies from its own realm, the frame's parent.
                post_from(&host, &source, reply);
            } else {
                resolve.call1(&JsValue::UNDEFINED, &data).unwrap();
            }
        });
        window
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
