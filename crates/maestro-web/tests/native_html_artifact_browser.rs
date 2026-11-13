//! HTML artifact bodies observed through the native document.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::browser_fixture_support::{object, set};
use crate::contributions_support::{
    Clipboard, Recorded, container, document, host, localized_view, published, select, text, view,
};
use crate::native_html_artifact_support::{
    LOGGING, QUIET, before, failing, frame, listed, logged, marker, srcdoc,
};
use maestro_models::{DiagnosticErrorInfo, SharedHandle};
use maestro_web::components::sandbox::runtime_bundle::BUNDLE;
use maestro_web::{
    HtmlArtifact,
    browser::values::BrowserSandboxValue,
    page::{DomContent as _, DomRoot, DomView},
};
use maestro_web::{Runtime, SandboxRuntimeProvider};
use std::{cell::Cell, rc::Rc};
use wasm_bindgen::{JsCast as _, JsValue, prelude::wasm_bindgen};
use web_sys::{Element, Node};

/// A provider whose every read fails the test, proving a detached body never consults it.
struct Unread;
impl SandboxRuntimeProvider<BrowserSandboxValue> for Unread {
    fn get_data(
        &self,
    ) -> Result<indexmap::IndexMap<String, BrowserSandboxValue>, DiagnosticErrorInfo> {
        panic!("detached body read provider data")
    }
    fn get_runtime(&self) -> Result<Runtime, DiagnosticErrorInfo> {
        panic!("detached body read the provider runtime")
    }
    fn get_description(&self) -> Result<String, DiagnosticErrorInfo> {
        panic!("detached body read the provider description")
    }
}
/// The listing's code node.
fn code(root: &DomRoot) -> Element {
    select(
        &root.node,
        ".maestro-html-source pre.maestro-text-artifact > code",
    )
}
/// The first node below the listing, whose identity shows the listing was not rebuilt.
fn first_token(root: &DomRoot) -> Node {
    code(root).first_child().unwrap()
}
/// Assert the empty state a fresh artifact starts with.
fn assert_empty(artifact: &HtmlArtifact) {
    assert_eq!(artifact.filename, "");
    assert_eq!(artifact.content(), "");
    assert!(artifact.runtime_providers.is_empty());
    assert!(artifact.sandbox_url_provider.is_none());
    assert!(artifact.sandbox_iframe_ref.borrow().is_none());
}
/// Mount a named artifact without connecting it, in preview with an empty listing, whose
/// provider and sandbox address fail the test when read.
fn detached(view: &DomView) -> (HtmlArtifact, DomRoot) {
    let mut artifact = HtmlArtifact::new();
    artifact.filename = "page.html".to_owned();
    artifact.runtime_providers.push(SharedHandle::new(Unread));
    artifact.sandbox_url_provider = Some(SharedHandle::new(
        || -> Result<String, DiagnosticErrorInfo> {
            panic!("detached body read the sandbox address")
        },
    ));
    let root = artifact.mount(view).unwrap();
    assert_eq!(text(&code(&root)), "");
    assert!(select(&root.node, ".maestro-html-source").has_attribute("hidden"));
    assert!(!select(&root.node, ".maestro-html-preview").has_attribute("hidden"));
    assert_eq!(code(&root).class_name(), "hljs language-html");
    (artifact, root)
}
/// Hostile, Unicode and control-character sources stay original and list as text only.
fn assert_literal_sources(artifact: &mut HtmlArtifact, root: &DomRoot, view: &DomView) {
    // Source, then the listing text after the page parser normalizes line breaks and drops NUL.
    for (source, listed) in [
        (
            "<script>alert(\"x\")</script><img src=x onerror=alert(1)>",
            "<script>alert(\"x\")</script><img src=x onerror=alert(1)>",
        ),
        ("<p>snow 雪😀</p>", "<p>snow 雪😀</p>"),
        (
            "\u{feff}<html>\r\n<&>\u{0}</html>",
            "\u{feff}<html>\n<&></html>",
        ),
        ("a\u{85}b\rc", "a\u{85}b\nc"),
    ] {
        artifact.set_content(source.to_owned()).unwrap();
        artifact.update(root, view).unwrap();
        assert_eq!(artifact.content(), source);
        assert_eq!(text(&code(root)), listed);
        let authored = code(root).query_selector_all(":not(span)").unwrap();
        assert_eq!(authored.length(), 0, "{source:?}");
    }
    assert!(artifact.sandbox_iframe_ref.borrow().is_none());
    let node: &Element = root.node.unchecked_ref();
    assert!(node.query_selector("iframe").unwrap().is_none());
}
/// Detached bodies keep original source, read no provider data and leave the source in place.
#[wasm_bindgen]
pub fn html_artifact_dom_source_is_literal_before_connection(import_module: &js_sys::Function) {
    let view = view();
    let _host = host(&view, import_module);
    let neighbour = HtmlArtifact::default();
    assert_empty(&HtmlArtifact::new());
    assert_empty(&neighbour);
    let (mut artifact, root) = detached(&view);
    assert!(!Rc::ptr_eq(
        &artifact.sandbox_iframe_ref,
        &neighbour.sandbox_iframe_ref
    ));
    assert_literal_sources(&mut artifact, &root, &view);
    assert_empty(&neighbour);
    // Selection needs a placed listing; the connected body then loads without reading the
    // removed provider or address.
    artifact.runtime_providers.clear();
    artifact.sandbox_url_provider = None;
    artifact.update(&root, &view).unwrap();
    container().append_child(&root.node).unwrap();
    let listing = code(&root);
    let token = first_token(&root);
    crate::contributions_support::page::select_children(&listing);
    artifact.set_content(artifact.content().to_owned()).unwrap();
    artifact.filename = "renamed.html".to_owned();
    artifact.update(&root, &view).unwrap();
    assert!(code(&root).is_same_node(Some(&listing)));
    assert!(first_token(&root).is_same_node(Some(&token)));
    assert!(crate::contributions_support::page::selection(&listing).1);
    artifact.set_content("<b>changed</b>".to_owned()).unwrap();
    artifact.update(&root, &view).unwrap();
    assert!(code(&root).is_same_node(Some(&listing)));
    assert_eq!(text(&code(&root)), "<b>changed</b>");
    assert!(code(&root).query_selector("b").unwrap().is_none());
}
/// A connected body loads once per connection, changed source or explicit run, never on logs,
/// equal or empty assignments or unrelated updates.
#[wasm_bindgen]
pub async fn html_artifact_dom_executes_once_per_source_or_reload(import_module: js_sys::Function) {
    let view = view();
    let _host = host(&view, &import_module);
    let reads = Rc::new(Cell::new(0));
    let mut artifact = HtmlArtifact::new();
    artifact.filename = "proof.html".to_owned();
    artifact.runtime_providers.push(marker("counted", &reads));
    artifact.set_content(LOGGING.to_owned()).unwrap();
    let root = artifact.mount(&view).unwrap();
    assert_eq!((reads.get(), frame(&root).is_some()), (0, false));
    let parent = container();
    parent.append_child(&root.node).unwrap();
    assert_eq!((reads.get(), frame(&root).is_some()), (1, true));
    logged(&root, &artifact, &view.i18n, "[log] one").await;
    artifact.update(&root, &view).unwrap();
    artifact.set_content(LOGGING.to_owned()).unwrap();
    assert_eq!(reads.get(), 1);
    assert_eq!(artifact.get_logs(&view.i18n).unwrap(), "[log] one");
    let guidance = "No logs for proof.html";
    artifact.set_content(QUIET.to_owned()).unwrap();
    assert_eq!(reads.get(), 2);
    assert_eq!(artifact.get_logs(&view.i18n).unwrap(), guidance);
    artifact.update(&root, &view).unwrap();
    assert_eq!(reads.get(), 2, "a log-free source is not run again");
    artifact.execute_content(artifact.content()).unwrap();
    assert_eq!(reads.get(), 3);
    artifact.set_content(String::new()).unwrap();
    assert_eq!(reads.get(), 3);
    artifact.execute_content("").unwrap();
    assert_eq!(reads.get(), 4, "an explicit empty run still loads");
    reload_current(&view);
    replace_unlisted_source(&view);
    reconnection(&parent, &root, &mut artifact, &reads).await;
    failure(&root, &mut artifact);
    parent.remove();
}
/// A retained reload reads each assignment before any presentation update, including failure.
fn reload_current(view: &DomView) {
    let reads = Rc::new(Cell::new(0));
    let mut artifact = HtmlArtifact::new();
    artifact.runtime_providers.push(marker("reload", &reads));
    artifact.set_content(QUIET.to_owned()).unwrap();
    let root = artifact.mount(view).unwrap();
    let parent = container();
    parent.append_child(&root.node).unwrap();
    let header = artifact.get_header_buttons(view).unwrap();
    let reload = select(&header.node, "button[title='Reload HTML']");
    for source in ["<p>new</p>", ""] {
        artifact.set_content(source.to_owned()).unwrap();
        reload.unchecked_ref::<web_sys::HtmlElement>().click();
        assert!(
            srcdoc(&root).ends_with(&format!(
                "{source}<script>window.__maestroComplete</script>"
            )),
            "reload did not use current source"
        );
    }
    let failed = Rc::new(Cell::new(true));
    artifact.runtime_providers = vec![SharedHandle::new(
        crate::native_html_artifact_support::Marker {
            name: "recovering",
            reads: Rc::clone(&reads),
            fails: Rc::clone(&failed),
        },
    )];
    artifact
        .set_content("<p>failed</p>".to_owned())
        .unwrap_err();
    let before = reads.get();
    failed.set(false);
    reload.unchecked_ref::<web_sys::HtmlElement>().click();
    assert_eq!(
        reads.get(),
        before + 1,
        "reload retries the failed assignment"
    );
    assert!(srcdoc(&root).ends_with("<p>failed</p><script>window.__maestroComplete</script>"));
    parent.remove();
}
/// Replacement compares current source, even when the old listing has not been updated.
fn replace_unlisted_source(view: &DomView) {
    let reads = Rc::new(Cell::new(0));
    let mut first = HtmlArtifact::new();
    first.set_content(QUIET.to_owned()).unwrap();
    let root = first.mount(view).unwrap();
    let parent = container();
    parent.append_child(&root.node).unwrap();
    let header = first.get_header_buttons(view).unwrap();
    first.set_content(LOGGING.to_owned()).unwrap();
    assert_eq!(listed(&root), QUIET);
    let mut replacement = HtmlArtifact::new();
    replacement
        .runtime_providers
        .push(marker("replacement", &reads));
    replacement.set_content(QUIET.to_owned()).unwrap();
    replacement.update(&root, view).unwrap();
    assert_eq!(
        reads.get(),
        1,
        "replacement loads against the current unlisted source"
    );
    select(&header.node, "button[title='Reload HTML']")
        .unchecked_ref::<web_sys::HtmlElement>()
        .click();
    assert_eq!(reads.get(), 2, "retained header reloads the replacement");
    replacement.update(&root, view).unwrap();
    assert_eq!(reads.get(), 2, "equal update remains suppressed");
    assert!(srcdoc(&root).ends_with("<p>quiet</p><script>window.__maestroComplete</script>"));
    parent.remove();
}
/// Detaching removes the frame and its route; reconnecting loads the source once more.
async fn reconnection(
    parent: &Element,
    root: &DomRoot,
    artifact: &mut HtmlArtifact,
    reads: &Rc<Cell<u32>>,
) {
    let view = view();
    let adapter = artifact.sandbox_iframe_ref.borrow().clone().unwrap();
    parent.remove_child(&root.node).unwrap();
    assert!(frame(root).is_none());
    assert!(artifact.sandbox_iframe_ref.borrow().is_none());
    artifact.set_content(LOGGING.to_owned()).unwrap();
    assert_eq!(reads.get(), 4, "a detached body does not load");
    artifact.update(root, &view).unwrap();
    parent.append_child(&root.node).unwrap();
    assert_eq!(reads.get(), 5);
    let live = artifact.sandbox_iframe_ref.borrow().clone().unwrap();
    assert!(
        Rc::ptr_eq(&live, &adapter),
        "reconnection keeps the adapter"
    );
    logged(root, artifact, &view.i18n, "[log] one").await;
}
/// A reached provider failure returns after the source and logs changed; updates do not retry.
fn failure(root: &DomRoot, artifact: &mut HtmlArtifact) {
    let view = view();
    let fails = Rc::new(Cell::new(0));
    artifact.runtime_providers = vec![failing(&fails)];
    let error = artifact.set_content(QUIET.to_owned()).unwrap_err();
    assert_eq!(error.to_string(), "provider failed");
    assert_eq!(artifact.content(), QUIET);
    assert_eq!(
        artifact.get_logs(&view.i18n).unwrap(),
        "No logs for proof.html"
    );
    artifact.update(root, &view).unwrap();
    assert_eq!(fails.get(), 1, "an unrelated update does not retry");
}
/// Loads read the current providers in order, and the frame address follows the configured
/// address, keeping the frame's own when none is configured.
#[wasm_bindgen]
pub async fn html_artifact_dom_forwards_current_providers_and_url(import_module: js_sys::Function) {
    let view = view();
    let _host = host(&view, &import_module);
    let reads = Rc::new(Cell::new(0));
    let mut artifact = HtmlArtifact::new();
    artifact.filename = "forward.html".to_owned();
    artifact.runtime_providers = vec![marker("first", &reads), marker("second", &reads)];
    artifact.set_content(QUIET.to_owned()).unwrap();
    let root = artifact.mount(&view).unwrap();
    let parent = container();
    parent.append_child(&root.node).unwrap();
    let document = srcdoc(&root);
    assert!(before(&document, "[\"first\",", "[\"second\","));
    assert!(before(&document, "runtime_first", "runtime_second"));
    assert!(document.contains("\"artifact-forward.html\""));
    assert!(document.ends_with("<p>quiet</p><script>window.__maestroComplete</script>"));
    assert!(!frame(&root).unwrap().has_attribute("src"));
    let header = artifact.get_header_buttons(&view).unwrap();
    addresses(&root, &mut artifact).await;
    providers(&root, &mut artifact);
    retarget(&root, &artifact, &reads, &header).await;
    parent.remove();
    address_before_connection().await;
}
/// A counting address provider for `path` below the page origin.
fn address(path: &'static str, reads: &Rc<Cell<u32>>) -> maestro_web::SandboxUrlProvider {
    let reads = Rc::clone(reads);
    SharedHandle::new(move || {
        reads.set(reads.get() + 1);
        Ok(format!("{}{path}", crate::contributions_support::origin()))
    })
}
/// An address changed while the body is detached reaches the body through an update, and its
/// first load runs at that address, whose page delivers the log through the body's consumer.
async fn address_before_connection() {
    let view = view();
    let (first, changed) = (Rc::default(), Rc::default());
    let mut artifact = HtmlArtifact::new();
    artifact.filename = "early.html".to_owned();
    artifact.sandbox_url_provider = Some(address("/sandbox.html?first", &first));
    artifact.set_content(LOGGING.to_owned()).unwrap();
    let root = artifact.mount(&view).unwrap();
    artifact.sandbox_url_provider = Some(address("/sandbox.html?changed", &changed));
    artifact.update(&root, &view).unwrap();
    let parent = container();
    parent.append_child(&root.node).unwrap();
    assert_eq!(
        (frame(&root).unwrap().src(), first.get(), changed.get()),
        (
            format!(
                "{}/sandbox.html?changed",
                crate::contributions_support::origin()
            ),
            0,
            1
        )
    );
    logged(&root, &artifact, &view.i18n, "[log] one").await;
    parent.remove();
}
/// Configured, changed, absent, frame-direct and failing addresses.
async fn addresses(root: &DomRoot, artifact: &mut HtmlArtifact) {
    let view = view();
    let (first, second, direct) = (Rc::default(), Rc::default(), Rc::default());
    artifact.sandbox_url_provider = Some(address("/sandbox.html", &first));
    artifact.execute_content(LOGGING).unwrap();
    assert_eq!(
        frame(root).unwrap().src(),
        format!("{}/sandbox.html", crate::contributions_support::origin())
    );
    logged(root, artifact, &view.i18n, "[log] one").await;
    artifact.sandbox_url_provider = Some(address("/sandbox.html?changed", &second));
    artifact.execute_content(QUIET).unwrap();
    assert!(
        frame(root)
            .unwrap()
            .src()
            .ends_with("/sandbox.html?changed")
    );
    artifact.sandbox_url_provider = None;
    artifact.execute_content(QUIET).unwrap();
    assert!(
        frame(root)
            .unwrap()
            .src()
            .ends_with("/sandbox.html?changed")
    );
    assert_eq!(
        (first.get(), second.get()),
        (1, 2),
        "an absent address keeps the frame's"
    );
    let adapter = artifact.sandbox_iframe_ref.borrow().clone().unwrap();
    adapter
        .sandbox_url_provider
        .replace(Some(address("/sandbox.html?direct", &direct)));
    artifact.execute_content(QUIET).unwrap();
    assert!(frame(root).unwrap().src().ends_with("/sandbox.html?direct"));
    assert_eq!(direct.get(), 1);
    artifact.sandbox_url_provider = Some(SharedHandle::new(|| {
        Err(DiagnosticErrorInfo::error("address failed"))
    }));
    let error = artifact.execute_content(QUIET).unwrap_err();
    assert_eq!(error.to_string(), "address failed");
    artifact.sandbox_url_provider = None;
    adapter.sandbox_url_provider.replace(None);
}
/// Replaced, emptied and failing provider lists reach the next load.
fn providers(root: &DomRoot, artifact: &mut HtmlArtifact) {
    let reads = Rc::new(Cell::new(0));
    artifact.runtime_providers = vec![marker("third", &reads)];
    artifact.execute_content(QUIET).unwrap();
    let document = srcdoc(root);
    assert!(document.contains("[\"third\",") && !document.contains("[\"first\","));
    artifact.runtime_providers.clear();
    artifact.execute_content(QUIET).unwrap();
    assert!(!srcdoc(root).contains("window[\"third\"]"));
    artifact.runtime_providers = vec![failing(&reads)];
    assert_eq!(
        artifact.execute_content(QUIET).unwrap_err().to_string(),
        "provider failed"
    );
    assert_eq!(reads.get(), 2);
}
/// Updating the root from another artifact moves the frame to it; its own operands load.
async fn retarget(
    root: &DomRoot,
    artifact: &HtmlArtifact,
    reads: &Rc<Cell<u32>>,
    header: &DomRoot,
) {
    let view = view();
    let adapter = artifact.sandbox_iframe_ref.borrow().clone().unwrap();
    let mut target = HtmlArtifact::new();
    target.filename = "target.html".to_owned();
    target.runtime_providers = vec![marker("target", reads)];
    target.set_content(LOGGING.to_owned()).unwrap();
    let previous_reads = reads.get();
    target.update(root, &view).unwrap();
    assert_eq!(reads.get(), previous_reads + 1, "replacement loads once");
    assert!(artifact.sandbox_iframe_ref.borrow().is_none());
    let moved = target.sandbox_iframe_ref.borrow().clone().unwrap();
    assert!(Rc::ptr_eq(&moved, &adapter));
    select(&header.node, "button[title='Reload HTML']")
        .unchecked_ref::<web_sys::HtmlElement>()
        .click();
    assert_eq!(
        reads.get(),
        previous_reads + 2,
        "retained header targets replacement"
    );
    let document = srcdoc(root);
    assert!(document.contains("\"artifact-target.html\"") && document.contains("runtime_target"));
    logged(root, &target, &view.i18n, "[log] one").await;
    assert_eq!(listed(root), LOGGING);
    artifact.execute_content(QUIET).unwrap();
    assert!(srcdoc(root).contains("\"artifact-target.html\""));
}
/// A provider editing, removing and adding message text in place before consumers see it.
struct Editing;
impl SandboxRuntimeProvider<BrowserSandboxValue> for Editing {
    fn handle_message(
        &self,
        message: BrowserSandboxValue,
        _: maestro_web::components::browser::sandbox::providers::SandboxRespond<
            BrowserSandboxValue,
        >,
    ) -> maestro_models::BoxFuture<Result<(), DiagnosticErrorInfo>> {
        let text = js_sys::Reflect::get(&message.0, &"text".into()).unwrap();
        let object: &js_sys::Object = message.0.unchecked_ref();
        match text.as_string().as_deref() {
            Some("edit-me") => {
                js_sys::Reflect::set(object, &"text".into(), &"edited".into()).unwrap();
            }
            Some("delete-me") => {
                js_sys::Reflect::delete_property(object, &"text".into()).unwrap();
            }
            _ if js_sys::Reflect::get(object, &"marker".into()).unwrap() == "add" => {
                js_sys::Reflect::set(object, &"text".into(), &"added".into()).unwrap();
            }
            _ => {}
        }
        Box::pin(async { Ok(()) })
    }
    fn get_data(
        &self,
    ) -> Result<indexmap::IndexMap<String, BrowserSandboxValue>, DiagnosticErrorInfo> {
        Ok(indexmap::IndexMap::new())
    }
    fn get_runtime(&self) -> Result<Runtime, DiagnosticErrorInfo> {
        Ok(Runtime::Content("() => {}".to_owned()))
    }
    fn get_description(&self) -> Result<String, DiagnosticErrorInfo> {
        Ok(String::new())
    }
}
/// Message methods posted after the value domains, as JSON text or `None` for a message
/// without one, with the line kind each selects.
const METHODS: [(Option<&str>, &str); 12] = [
    (Some("\"log\""), "log"),
    (Some("\"warn\""), "log"),
    (Some("\"info\""), "log"),
    (Some("\"debug\""), "log"),
    (Some("\"error\""), "error"),
    (None, "log"),
    (Some("null"), "log"),
    (Some("42"), "log"),
    (Some("true"), "log"),
    (Some("{}"), "log"),
    (Some("[]"), "log"),
    (Some("\"ERROR\""), "log"),
];
/// An artifact document that announces it listens, then posts each message of a list its
/// parent sends back to the parent with the sandbox identifier.
const ECHO: &str = "<script>addEventListener('message', event => { if (event.source === parent && Array.isArray(event.data)) for (const message of event.data) parent.postMessage({...message, sandboxId: window.sandboxId}, '*'); }); parent.postMessage({type: 'echo-ready'}, '*');</script>";
/// The raw messages the document posts: every recorded value domain, ignored message types,
/// every method kind, then the texts a provider edits in place.
fn messages(cases: &[crate::native_artifact_console_support::Value]) -> js_sys::Array {
    let console = |key: &str, value: &JsValue| -> JsValue {
        object(&[("type", &"console".into()), (key, value)]).into()
    };
    let messages = js_sys::Array::new();
    for case in cases {
        messages.push(&if case.kind == "missing" {
            object(&[("type", &"console".into())]).into()
        } else {
            let operand =
                crate::native_artifact_console_support::operand(&case.kind, &case.operand);
            console("text", &operand)
        });
    }
    messages.push(&object(&[("text", &"ignored".into())]));
    for kind in [
        "null",
        "\"download\"",
        "\"console-other\"",
        "17",
        "false",
        "{}",
        "[]",
        "\"Console\"",
    ] {
        let kind = js_sys::JSON::parse(kind).unwrap();
        messages.push(&object(&[("type", &kind), ("text", &"ignored".into())]));
    }
    for (method, _) in METHODS {
        let line = console("text", &"line".into());
        if let Some(method) = method {
            set(&line, "method", &js_sys::JSON::parse(method).unwrap());
        }
        messages.push(&line);
    }
    messages.push(&console("text", &"edit-me".into()));
    messages.push(&console("text", &"delete-me".into()));
    messages.push(&console("marker", &"add".into()));
    messages
}
/// Only console messages append, keeping each raw text, the error kind only for `error`, and
/// the frame lives exactly as long as the connected root.
#[wasm_bindgen]
pub async fn html_artifact_dom_routes_logs_and_releases_its_frame(import_module: js_sys::Function) {
    let view = view();
    let _host = host(&view, &import_module);
    guidance(&view);
    let cases = crate::native_artifact_console_support::cases().values;
    let mut artifact = HtmlArtifact::new();
    artifact.filename = "routes.html".to_owned();
    artifact.runtime_providers.push(SharedHandle::new(Editing));
    artifact.set_content(ECHO.to_owned()).unwrap();
    let root = artifact.mount(&view).unwrap();
    let parent = container();
    let listening = crate::sandbox_execution_support::Fixture::watch("echo-ready");
    parent.append_child(&root.node).unwrap();
    crate::contributions_support::settle(listening.into())
        .await
        .unwrap();
    // The values are built here; the frame posts their structured clones as its own messages.
    frame(&root)
        .unwrap()
        .content_window()
        .unwrap()
        .post_message(&messages(&cases), "*")
        .unwrap();
    let (returned, painted) = expected_lines(&cases);
    logged(&root, &artifact, &view.i18n, &returned.join("\n")).await;
    crate::native_html_artifact_support::expand(&root);
    assert_eq!(crate::native_html_artifact_support::painted(&root), painted);
    growth(&root, &mut artifact).await;
    parent.remove();
    lifetime().await;
}
/// The returned and painted lines the messages page produces, in order.
fn expected_lines(
    cases: &[crate::native_artifact_console_support::Value],
) -> (Vec<String>, Vec<String>) {
    let mut returned = Vec::new();
    let mut painted = Vec::new();
    for case in cases {
        let (copy, paint) = if case.kind == "date" {
            let date = crate::native_artifact_console_support::operand(&case.kind, &case.operand);
            let line = format!(
                "[log] {}",
                crate::native_artifact_console_support::native_string(&date)
            );
            (line.clone(), line)
        } else {
            (case.copy.clone().unwrap(), case.paint.clone().unwrap())
        };
        returned.push(copy);
        painted.push(paint);
    }
    for (_, kind) in METHODS {
        returned.push(format!("[{kind}] line"));
        painted.push(format!("[{kind}] line"));
    }
    for (copy, paint) in [
        ("[log] edited", "[log] edited"),
        ("[log] undefined", "[log]"),
        ("[log] added", "[log] added"),
    ] {
        returned.push(copy.to_owned());
        painted.push(paint.to_owned());
    }
    (returned, painted)
}
/// Changed sources reset the logs; 16, 32 and 64 messages arrive whole and in order, copy as
/// one text, and twice the messages publish at most twice the bytes outside the frame.
async fn growth(root: &DomRoot, artifact: &mut HtmlArtifact) {
    let view = view();
    let clipboard = Recorded::install(Clipboard::Accept);
    let preview = select(&root.node, ".maestro-html-preview");
    let mut bytes = Vec::new();
    for count in [16, 32, 64] {
        let take = published(&preview, ".maestro-html-frame");
        artifact
            .set_content(format!(
                "<script type=\"module\">for (let i = 0; i < {count}; i++) window.parent.postMessage({{type: 'console', text: 'line-' + i, sandboxId: window.sandboxId}}, '*');</script>"
            ))
            .unwrap();
        let expected = (0..count)
            .map(|index| format!("[log] line-{index}"))
            .collect::<Vec<_>>()
            .join("\n");
        logged(root, artifact, &view.i18n, &expected).await;
        bytes.push(take());
        select(&root.node, ".maestro-artifact-console copy-button button")
            .unchecked_ref::<web_sys::HtmlElement>()
            .click();
        assert_eq!(clipboard.texts().last(), Some(&expected));
    }
    web_sys::console::log_1(&format!("MEASURE html log bytes {bytes:?}").into());
    assert!(
        bytes[1] <= 2 * bytes[0] && bytes[2] <= 4 * bytes[0],
        "messages published superlinear bytes: {bytes:?}"
    );
}
/// Empty logs name the file literally, replacing only the first placeholder, in both languages.
fn guidance(view: &DomView) {
    let mut artifact = HtmlArtifact::new();
    artifact.filename = "literal$&$`$'雪.html".to_owned();
    assert_eq!(
        artifact.get_logs(&view.i18n).unwrap(),
        "No logs for literal$&$`$'雪.html"
    );
    let german = localized_view(view, "de");
    assert_eq!(
        artifact.get_logs(&german.i18n).unwrap(),
        "Keine Logs für literal$&$`$'雪.html"
    );
    let english = localized_view(view, "en");
    let translations = english.i18n.get_translations();
    let _ = translations.borrow()["en"].borrow_mut().insert(
        "No logs for {filename}".into(),
        maestro_web::TranslationValue::Text("First {filename} / {filename}".into()),
    );
    artifact.filename = "literal$&雪.html".to_owned();
    assert_eq!(
        artifact.get_logs(&english.i18n).unwrap(),
        "First literal$&雪.html / {filename}"
    );
}
/// The public frame reference follows connection, and releasing one root leaves its retained
/// adapter handle released and a differently named sibling working.
async fn lifetime() {
    let view = view();
    let parent = container();
    let provider = marker("released", &Rc::default());
    let released = Rc::downgrade(&provider);
    let mut first = HtmlArtifact::new();
    first.runtime_providers.push(provider);
    first.filename = "first.html".to_owned();
    first.set_content(LOGGING.to_owned()).unwrap();
    let mut sibling = HtmlArtifact::new();
    sibling.filename = "sibling.html".to_owned();
    sibling.set_content(LOGGING.to_owned()).unwrap();
    let root = first.mount(&view).unwrap();
    let sibling_root = sibling.mount(&view).unwrap();
    parent.append_child(&root.node).unwrap();
    parent.append_child(&sibling_root.node).unwrap();
    logged(&root, &first, &view.i18n, "[log] one").await;
    let adapter = first.sandbox_iframe_ref.borrow().clone().unwrap();
    parent.remove_child(&root.node).unwrap();
    assert!(first.sandbox_iframe_ref.borrow().is_none() && frame(&root).is_none());
    parent.append_child(&root.node).unwrap();
    let live = first.sandbox_iframe_ref.borrow().clone().unwrap();
    assert!(Rc::ptr_eq(&live, &adapter) && frame(&root).is_some());
    drop(live);
    parent.remove_child(&root.node).unwrap();
    drop(root);
    first.runtime_providers.clear();
    assert!(
        released.upgrade().is_none(),
        "released body retains its provider"
    );
    assert!(first.sandbox_iframe_ref.borrow().is_none());
    assert_eq!(
        Rc::strong_count(&adapter),
        1,
        "only the retained handle remains"
    );
    adapter.remove().unwrap();
    assert!(frame(&sibling_root).is_some());
    logged(&sibling_root, &sibling, &view.i18n, "[log] one").await;
    sibling.execute_content(sibling.content()).unwrap();
    logged(&sibling_root, &sibling, &view.i18n, "[log] one\n[log] one").await;
    parent.remove();
}
/// Interleaved bodies and copy controls keep their own operands, untaken operands never reach
/// a later default element, and releasing one body leaves the other usable.
#[wasm_bindgen]
pub async fn html_artifact_dom_nested_creation_keeps_controllers(import_module: js_sys::Function) {
    let view = view();
    let _host = host(&view, &import_module);
    let german = localized_view(&view, "de");
    let parent = container();
    let (reads_a, reads_b) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
    let mut first = HtmlArtifact::new();
    first.filename = "a.html".to_owned();
    first.runtime_providers.push(marker("a", &reads_a));
    first.set_content(LOGGING.to_owned()).unwrap();
    let root_a = first.mount(&view).unwrap();
    parent.append_child(&root_a.node).unwrap();
    logged(&root_a, &first, &view.i18n, "[log] one").await;
    crate::native_html_artifact_support::expand(&root_a);
    let mut second = HtmlArtifact::new();
    second.filename = "b.html".to_owned();
    second.runtime_providers.push(marker("b", &reads_b));
    second.set_content(LOGGING.to_owned()).unwrap();
    let root_b = second.mount(&german).unwrap();
    parent.append_child(&root_b.node).unwrap();
    logged(&root_b, &second, &german.i18n, "[log] one").await;
    crate::native_html_artifact_support::expand(&root_b);
    for (root, title, own, other) in [
        (&root_a, "Copy logs", "runtime_a", "runtime_b"),
        (&root_b, "Logs kopieren", "runtime_b", "runtime_a"),
    ] {
        let copy = select(&root.node, "copy-button");
        assert_eq!(copy.get_attribute("title").as_deref(), Some(title));
        assert!(srcdoc(root).contains(own) && !srcdoc(root).contains(other));
    }
    assert_eq!((reads_a.get(), reads_b.get()), (1, 1));
    attributes(&parent, &root_a, &reads_a);
    default_released(&view, &parent).await;
    parent.remove_child(&root_a.node).unwrap();
    drop(root_a);
    assert!(first.sandbox_iframe_ref.borrow().is_none());
    second.execute_content(second.content()).unwrap();
    assert_eq!(reads_b.get(), 2);
    logged(&root_b, &second, &german.i18n, "[log] one\n[log] one").await;
    parent.remove();
}
/// A disconnected default element releases its controller-owned descendants.
async fn default_released(view: &DomView, parent: &Element) {
    let released = untaken(view, parent);
    let window = web_sys::window().unwrap();
    let gc = js_sys::Reflect::get(&window, &"gc".into())
        .unwrap()
        .unchecked_into::<js_sys::Function>();
    for _ in 0..4 {
        crate::contributions_support::frames(1).await;
        gc.call0(&window).unwrap();
    }
    assert!(
        released.deref().is_none(),
        "default element retains its body subtree"
    );
}
/// The filename attribute renames the next load's sandbox; a content attribute is not source.
fn attributes(parent: &Element, root: &DomRoot, reads: &Rc<Cell<u32>>) {
    let node: &Element = root.node.unchecked_ref();
    node.set_attribute("filename", "renamed.html").unwrap();
    node.set_attribute("content", "<p>attribute</p>").unwrap();
    let property = js_sys::Reflect::get(node, &"filename".into()).unwrap();
    assert_eq!(property.as_string().as_deref(), Some("renamed.html"));
    parent.remove_child(node).unwrap();
    parent.append_child(node).unwrap();
    assert_eq!(reads.get(), 2);
    assert!(srcdoc(root).contains("\"artifact-renamed.html\""));
    assert_eq!(listed(root), LOGGING);
}
/// A body mounted into a document without element definitions leaves its operands untaken, and
/// a later default element starts empty.
fn untaken(view: &DomView, parent: &Element) -> js_sys::WeakRef<Element> {
    let inert = web_sys::DomParser::new()
        .unwrap()
        .parse_from_string(
            "<!doctype html><html></html>",
            web_sys::SupportedType::TextHtml,
        )
        .unwrap();
    let stale_reads = Rc::new(Cell::new(0));
    let mut stale = HtmlArtifact::new();
    stale.filename = "stale.html".to_owned();
    stale.runtime_providers.push(marker("stale", &stale_reads));
    stale.set_content("<p>stale</p>".to_owned()).unwrap();
    let inert_view = DomView {
        document: inert,
        ..view.clone()
    };
    let inert_root = stale.mount(&inert_view).unwrap();
    let accessor = js_sys::Reflect::get(&inert_root.node, &"filename".into()).unwrap();
    assert!(
        accessor.is_undefined(),
        "an inert document constructs no element"
    );
    let fresh = document().create_element("html-artifact").unwrap();
    fresh.set_attribute("filename", "default.html").unwrap();
    parent.append_child(&fresh).unwrap();
    assert_eq!(text(&select(&fresh, ".maestro-html-source code")), "");
    assert!(
        select(&fresh, ".maestro-html-frame")
            .first_child()
            .is_none()
    );
    assert_eq!(stale_reads.get(), 0);
    fresh.remove();
    js_sys::WeakRef::new(&fresh)
}
/// A long logging source whose listing overflows a bounded body.
fn long_source() -> String {
    format!(
        "{}<script>console.log(\"one\")</script>",
        "<p>line</p>\n".repeat(80)
    )
}
/// The preview and source buttons of a header, in order.
fn toggles(header: &DomRoot) -> (web_sys::HtmlElement, web_sys::HtmlElement) {
    let buttons = header
        .node
        .unchecked_ref::<Element>()
        .query_selector_all(".maestro-toggle-button")
        .unwrap();
    let at = |index| buttons.item(index).unwrap().unchecked_into();
    (at(0), at(1))
}
/// Measure the HTML header's eight-pixel space between the toggle and reload.
fn assert_header_gap(header: &DomRoot) {
    let left = header
        .node
        .first_child()
        .unwrap()
        .unchecked_into::<Element>()
        .get_bounding_client_rect();
    let right = header
        .node
        .child_nodes()
        .item(1)
        .unwrap()
        .unchecked_into::<Element>()
        .get_bounding_client_rect();
    assert!(
        (right.left() - left.right() - 8.0).abs() < 0.1,
        "HTML header gap must be 8px"
    );
}
/// Whether a body shows its source listing rather than its preview.
fn shows_source(root: &DomRoot) -> bool {
    let preview = select(&root.node, ".maestro-html-preview");
    let source = select(&root.node, ".maestro-html-source");
    assert_eq!(
        preview.has_attribute("hidden"),
        !source.has_attribute("hidden")
    );
    preview.get_bounding_client_rect().width() <= 0.0
}
/// Header selections switch the presentation of one body while its frame, listing,
/// selection, scroll and single load stay in place.
#[wasm_bindgen]
pub async fn html_artifact_dom_keeps_frame_across_mode_changes(import_module: js_sys::Function) {
    let view = view();
    let _host = host(&view, &import_module);
    let parent = container();
    parent
        .set_attribute("style", "display:block;height:160px")
        .unwrap();
    let reads = Rc::new(Cell::new(0));
    let mut artifact = HtmlArtifact::new();
    artifact.filename = "modes.html".to_owned();
    artifact.runtime_providers.push(marker("modes", &reads));
    artifact.set_content(long_source()).unwrap();
    let root = artifact.mount(&view).unwrap();
    parent.append_child(&root.node).unwrap();
    let header = artifact.get_header_buttons(&view).unwrap();
    container().append_child(&header.node).unwrap();
    assert_header_gap(&header);
    logged(&root, &artifact, &view.i18n, "[log] one").await;
    // The load and the header's standalone preparation each read the provider once.
    assert_eq!(reads.get(), 2);
    let loaded = frame(&root).unwrap();
    assert!(!shows_source(&root));
    let (preview_button, source_button) = toggles(&header);
    source_button.click();
    assert!(shows_source(&root));
    assert!(frame(&root).unwrap().is_same_node(Some(&loaded)));
    source_mode(&root, &mut artifact);
    source_paint(&parent).await;
    preview_button.focus().unwrap();
    crate::contributions_support::driver("driverPress", "Enter").await;
    assert!(!shows_source(&root));
    assert!(frame(&root).unwrap().is_same_node(Some(&loaded)));
    source_button.click();
    assert!(shows_source(&root));
    crate::support::reading_paint::observe("html-source-again").await;
    preview_button.click();
    assert!(!shows_source(&root));
    assert_eq!(reads.get(), 2, "changing the presentation never loads");
    sibling_modes(&view, &root);
    parent.remove();
}
/// The shown listing scrolls inside the bounded body and keeps its nodes, selection and scroll
/// through an unrelated update.
fn source_mode(root: &DomRoot, artifact: &mut HtmlArtifact) {
    let view = view();
    let source = select(&root.node, ".maestro-html-source");
    source.set_scroll_top(40);
    let scrolled = source.scroll_top();
    assert!(scrolled > 0, "the bounded listing scrolls");
    let listing = code(root);
    crate::contributions_support::page::select_children(&listing);
    artifact.filename = "renamed.html".to_owned();
    artifact.update(root, &view).unwrap();
    assert!(
        shows_source(root),
        "an unrelated update keeps the presentation"
    );
    assert!(code(root).is_same_node(Some(&listing)));
    assert!(crate::contributions_support::page::selection(&listing).1);
    assert_eq!(source.scroll_top(), scrolled);
}
/// In both brand modes, the shown listing paints its background role over its box and keeps its
/// scrolled text inside the bounded body.
async fn source_paint(parent: &Element) {
    let view = view();
    crate::contributions_support::page::deselect();
    for mode in [
        maestro_theme::BrandMode::Dark,
        maestro_theme::BrandMode::Light,
    ] {
        let presentation = view.pack.presentation(mode).unwrap();
        let style = parent.unchecked_ref::<web_sys::HtmlElement>().style();
        for (key, value) in presentation.css_properties() {
            style.set_property(&key, &value).unwrap();
        }
        crate::support::reading_paint::publish_colors(&presentation.colors);
        crate::support::reading_paint::observe("html-source").await;
    }
}
/// A differently named sibling switches only its own presentation.
fn sibling_modes(view: &DomView, first: &DomRoot) {
    let mut other = HtmlArtifact::new();
    other.filename = "other.html".to_owned();
    other.set_content(QUIET.to_owned()).unwrap();
    let root = other.mount(view).unwrap();
    let parent = container();
    parent.append_child(&root.node).unwrap();
    let header = other.get_header_buttons(view).unwrap();
    parent.append_child(&header.node).unwrap();
    toggles(&header).1.click();
    assert!(shows_source(&root) && !shows_source(first));
    let reopened = other.get_header_buttons(view).unwrap();
    let selected = toggles(&reopened).1;
    assert!(selected.class_name().contains("maestro-toggle-selected"));
    parent.remove();
}
/// The header row's controls in order.
fn controls(header: &DomRoot) -> Vec<Element> {
    std::iter::successors(
        header.node.unchecked_ref::<Element>().first_element_child(),
        Element::next_element_sibling,
    )
    .collect()
}
/// Header control titles in order: preview, source, reload, copy, download.
fn titles(header: &DomRoot) -> Vec<String> {
    let (preview, source) = toggles(header);
    let row = controls(header);
    assert_eq!(row.len(), 4);
    let title = |index: usize| row[index].get_attribute("title").unwrap();
    vec![
        preview.title(),
        source.title(),
        title(1),
        title(2),
        title(3),
    ]
}
/// Header titles in both languages.
fn localized_titles(view: &DomView) {
    for (language, expected) in [
        (
            "en",
            [
                "Preview",
                "Code",
                "Reload HTML",
                "Copy HTML",
                "Download HTML",
            ],
        ),
        (
            "de",
            [
                "Vorschau",
                "Code",
                "HTML neu laden",
                "HTML kopieren",
                "HTML herunterladen",
            ],
        ),
    ] {
        let localized = localized_view(view, language);
        let header = HtmlArtifact::new().get_header_buttons(&localized).unwrap();
        assert_eq!(titles(&header), expected);
    }
}
/// The header's control at `index` in the row, or its inner button for the copy element.
fn control(header: &DomRoot, index: usize) -> web_sys::HtmlElement {
    let child = controls(header).swap_remove(index);
    child
        .query_selector("button")
        .unwrap()
        .unwrap_or(child)
        .unchecked_into()
}
/// Headers keep the source they were built from; downloads save original bytes or the
/// standalone document; completion follows the whole source.
#[wasm_bindgen]
pub async fn html_artifact_dom_source_copy_and_standalone_download(
    import_module: js_sys::Function,
) {
    let view = view();
    let _host = host(&view, &import_module);
    localized_titles(&view);
    let fixture = crate::session_download_browser::Fixture::default();
    let clipboard = crate::contributions_support::Recorded::install(
        crate::contributions_support::Clipboard::Accept,
    );
    let mut artifact = HtmlArtifact::new();
    artifact.filename = "literal$&雪.html".to_owned();
    artifact.set_content("test".to_owned()).unwrap();
    let parent = container();
    let captured = artifact.get_header_buttons(&view).unwrap();
    parent.append_child(&captured.node).unwrap();
    fixture
        .saved(
            &control(&captured, 3),
            b"test",
            "text/html",
            "literal$&雪.html",
        )
        .await;
    artifact.set_content("<p>edited</p>".to_owned()).unwrap();
    let edited = artifact.get_header_buttons(&view).unwrap();
    parent.append_child(&edited.node).unwrap();
    for (header, text) in [(&captured, "test"), (&edited, "<p>edited</p>")] {
        let before = clipboard.texts().len();
        control(header, 2).click();
        crate::contributions_support::frames(1).await;
        assert_eq!(clipboard.texts()[before..], [text.to_owned()]);
    }
    mounted_download(&fixture, &mut artifact).await;
    completion().await;
    parent.remove();
}
/// A connected body's header saves the standalone document, and reload loads the body's
/// current source after clearing its logs.
async fn mounted_download(
    fixture: &crate::session_download_browser::Fixture,
    artifact: &mut HtmlArtifact,
) {
    let view = view();
    let reads = Rc::new(Cell::new(0));
    artifact.runtime_providers.push(marker("saved", &reads));
    artifact.set_content(MARKING.to_owned()).unwrap();
    let root = artifact.mount(&view).unwrap();
    let parent = container();
    parent.append_child(&root.node).unwrap();
    logged(&root, artifact, &view.i18n, "[log] one").await;
    let header = artifact.get_header_buttons(&view).unwrap();
    parent.append_child(&header.node).unwrap();
    let adapter = artifact.sandbox_iframe_ref.borrow().clone().unwrap();
    let standalone = adapter
        .prepare_html_document(
            "artifact-literal$&雪.html",
            MARKING,
            &artifact.runtime_providers,
            Some(
                maestro_web::components::sandboxed_iframe::PrepareHtmlOptions {
                    is_html_artifact: true,
                    is_standalone: true,
                },
            ),
        )
        .unwrap();
    assert_eq!(
        authored(&standalone),
        MARKING,
        "the saved source has no completion"
    );
    assert!(!standalone.contains("sendRuntimeMessage"));
    fixture
        .saved(
            &control(&header, 3),
            standalone.as_bytes(),
            "text/html",
            "literal$&雪.html",
        )
        .await;
    standalone_runs(&standalone).await;
    // The connection, both standalone preparations and the changed source each read once.
    artifact.set_content(QUIET.to_owned()).unwrap();
    artifact.update(&root, &view).unwrap();
    assert_eq!(reads.get(), 4);
    control(&header, 1).click();
    assert_eq!(reads.get(), 5, "reload loads once");
    assert!(srcdoc(&root).ends_with("<p>quiet</p><script>window.__maestroComplete</script>"));
    assert_eq!(
        artifact.get_logs(&view.i18n).unwrap(),
        "No logs for literal$&雪.html"
    );
    parent.remove();
}
/// A source that writes one log line and marks its own page when it runs.
const MARKING: &str = "<html><body>proof<script>console.log(\"one\");document.body.append(\" ran\")</script></body></html>";
/// The saved standalone document runs on its own when a browser opens it: its globals and
/// provider runtime run with its sandbox name, and its authored script marks the page.
async fn standalone_runs(saved: &str) {
    let bytes = js_sys::Array::of1(&saved.into());
    let options = web_sys::BlobPropertyBag::new();
    // A browser opening a saved file that declares no encoding detects it from the bytes; the
    // page states the UTF-8 they were saved in, which an object URL would not detect.
    options.set_type("text/html;charset=utf-8");
    let blob = web_sys::Blob::new_with_str_sequence_and_options(&bytes, &options).unwrap();
    let url = web_sys::Url::create_object_url_with_blob(&blob).unwrap();
    let opened: web_sys::HtmlIFrameElement = document()
        .create_element("iframe")
        .unwrap()
        .unchecked_into();
    let loaded = crate::sandbox_loading_support::native::loaded(&opened);
    opened.set_src(&url);
    let parent = container();
    parent.append_child(&opened).unwrap();
    loaded.await;
    web_sys::Url::revoke_object_url(&url).unwrap();
    let window = crate::sandbox_loading_support::native::window_of(&opened);
    let global = |name: &str| js_sys::Reflect::get(&window, &name.into()).unwrap();
    let observed = (
        global("sandboxId").as_string(),
        global("saved").as_string(),
        global("runtime_saved").as_bool(),
        opened
            .content_document()
            .unwrap()
            .body()
            .unwrap()
            .inner_text(),
    );
    assert_eq!(
        observed,
        (
            Some("artifact-literal$&雪.html".to_owned()),
            Some("saved".to_owned()),
            Some(true),
            "proof ran".to_owned()
        ),
        "the saved document runs on its own"
    );
    parent.remove();
}
/// One recorded completion case: the source, its completed document and what the page did.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Completion {
    /// Original source.
    source: String,
    /// Source with the completion signal appended.
    html: String,
    /// Completion signals that ran.
    completions: u32,
    /// Script errors the page reported.
    errors: u32,
    /// Completion positions and document readiness, in order.
    events: serde_json::Value,
    /// Body element names when loading finished.
    body: Vec<String>,
}
/// Recorded completion cases.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Completions {
    /// Every case in order.
    cases: Vec<Completion>,
}
/// A provider runtime that records each completion with its position and readiness, script
/// errors and loading, then posts them as one console line after the page loads.
///
/// A provider runs as a classic script before the document's own scripts. The frame's renderer
/// is a separate target the browser driver learns of only after its document has run, so no
/// observer from outside the frame can be in place before those scripts.
const PROBE: &str = r"() => {
  const complete = window.complete;
  const events = [];
  let errors = 0;
  window.events = events;
  window.complete = function (...args) {
    const script = document.currentScript;
    events.push({
      completion: true,
      ready: document.readyState,
      parent: script.parentElement.tagName,
      previous: script.previousElementSibling?.tagName || null,
    });
    return complete.apply(this, args);
  };
  // Counted syntax errors are handled here, so the page reports none of its own.
  window.addEventListener('error', event => {
    if (event.error?.name === 'SyntaxError') errors += 1;
    event.preventDefault();
  });
  document.addEventListener('DOMContentLoaded', () => events.push('dom-ready'));
  window.addEventListener('load', () => window.parent.postMessage({
    type: 'console',
    sandboxId: window.sandboxId,
    text: JSON.stringify({
      events,
      errors,
      completions: events.filter(event => event && event.completion).length,
      body: [...(document.body?.children || [])].map(node => node.tagName),
    }),
  }, '*'));
}";
/// A provider whose runtime is [`PROBE`].
struct Probe;
impl SandboxRuntimeProvider<BrowserSandboxValue> for Probe {
    fn get_data(
        &self,
    ) -> Result<indexmap::IndexMap<String, BrowserSandboxValue>, DiagnosticErrorInfo> {
        Ok(indexmap::IndexMap::new())
    }
    fn get_runtime(&self) -> Result<Runtime, DiagnosticErrorInfo> {
        Ok(Runtime::Content(PROBE.to_owned()))
    }
    fn get_description(&self) -> Result<String, DiagnosticErrorInfo> {
        Ok(String::new())
    }
}
/// The authored part of a prepared document, without the injected runtime.
fn authored(document: &str) -> String {
    let start = document.find("<style>\nhtml, body {").unwrap();
    let end_marker = format!("<script>{BUNDLE}</script>");
    let end = document[start..].find(&end_marker).unwrap() + start + end_marker.len();
    format!("{}{}", &document[..start], &document[end..])
}
/// The completion signal follows the whole source exactly once, where the page parser places
/// it, at the same moment of loading; malformed tails stay unrepaired.
async fn completion() {
    let view = view();
    let cases: Completions =
        serde_json::from_str(include_str!("fixtures/html_artifact_completion.json")).unwrap();
    let mut artifact = HtmlArtifact::new();
    artifact.filename = "completion.html".to_owned();
    artifact.runtime_providers.push(SharedHandle::new(Probe));
    let root = artifact.mount(&view).unwrap();
    let parent = container();
    parent.append_child(&root.node).unwrap();
    for case in cases.cases {
        artifact.set_content(case.source.clone()).unwrap();
        if case.source.is_empty() {
            artifact.execute_content("").unwrap();
        }
        assert_eq!(authored(&srcdoc(&root)), case.html);
        crate::contributions_support::until(&root.node, || {
            artifact
                .get_logs(&view.i18n)
                .unwrap()
                .starts_with("[log] {")
        })
        .await;
        let logs = artifact.get_logs(&view.i18n).unwrap();
        let observed: serde_json::Value = serde_json::from_str(&logs["[log] ".len()..]).unwrap();
        assert_eq!(
            observed["completions"], case.completions,
            "{:?}",
            case.source
        );
        assert_eq!(observed["errors"], case.errors, "{:?}", case.source);
        assert_eq!(observed["events"], case.events, "{:?}", case.source);
        assert_eq!(
            observed["body"],
            serde_json::json!(case.body),
            "{:?}",
            case.source
        );
    }
    parent.remove();
}
