//! HTML artifact bodies observed through the native document.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::contributions_support::{
    call, container, document, host, localized_view, page_module, select, selection, text, view,
};
use crate::native_html_artifact_support::{
    LOGGING, QUIET, before, failing, frame, listed, logged, marker, srcdoc,
};
use maestro_app::{DiagnosticErrorInfo, SandboxRuntimeProvider, SharedHandle};
use maestro_web::{
    browser::sandbox_value::BrowserSandboxValue,
    dom::{DomContent as _, DomRoot, DomView},
    tools::artifacts::html_artifact::HtmlArtifact,
};
use std::{cell::Cell, rc::Rc};
use wasm_bindgen::{JsCast as _, prelude::wasm_bindgen};
use web_sys::{Element, Node};

/// A provider whose every read fails the test, proving a detached body never consults it.
struct Unread;
impl SandboxRuntimeProvider<BrowserSandboxValue> for Unread {
    fn get_data(
        &self,
    ) -> Result<indexmap::IndexMap<String, BrowserSandboxValue>, DiagnosticErrorInfo> {
        panic!("detached body read provider data")
    }
    fn get_runtime(&self) -> Result<String, DiagnosticErrorInfo> {
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
pub async fn html_artifact_dom_source_is_literal_before_connection(
    import_module: js_sys::Function,
) {
    let view = view();
    let _host = host(&view, &import_module);
    let page = page_module(&import_module).await;
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
    call(&page, "select", &js_sys::Array::of1(&listing)).await;
    artifact.set_content(artifact.content().to_owned()).unwrap();
    artifact.filename = "renamed.html".to_owned();
    artifact.update(&root, &view).unwrap();
    assert!(code(&root).is_same_node(Some(&listing)));
    assert!(first_token(&root).is_same_node(Some(&token)));
    assert!(selection(&page, &listing).await.1);
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
    let page = page_module(&import_module).await;
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
    logged(&page, &root, &artifact, &view.i18n, "[log] one").await;
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
    reconnection(&page, &parent, &root, &mut artifact, &reads).await;
    failure(&root, &mut artifact);
    parent.remove();
}
/// Detaching removes the frame and its route; reconnecting loads the source once more.
async fn reconnection(
    page: &wasm_bindgen::JsValue,
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
    logged(page, root, artifact, &view.i18n, "[log] one").await;
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
    let page = page_module(&import_module).await;
    let reads = Rc::new(Cell::new(0));
    let mut artifact = HtmlArtifact::new();
    artifact.filename = "forward.html".to_owned();
    artifact.runtime_providers = vec![marker("first", &reads), marker("second", &reads)];
    artifact.set_content(QUIET.to_owned()).unwrap();
    let root = artifact.mount(&view).unwrap();
    let parent = container();
    parent.append_child(&root.node).unwrap();
    let document = srcdoc(&root);
    assert!(before(&document, "window[\"first\"]", "window[\"second\"]"));
    assert!(before(&document, "runtime_first", "runtime_second"));
    assert!(document.contains("\"artifact-forward.html\""));
    assert!(
        document.ends_with("<p>quiet</p><script>if (window.complete) window.complete();</script>")
    );
    assert!(!frame(&root).unwrap().has_attribute("src"));
    addresses(&page, &root, &mut artifact).await;
    providers(&root, &mut artifact);
    retarget(&page, &root, &artifact, &reads).await;
    parent.remove();
}
/// A counting address provider for `path` below the page origin.
fn address(path: &'static str, reads: &Rc<Cell<u32>>) -> maestro_app::SandboxUrlProvider {
    let reads = Rc::clone(reads);
    SharedHandle::new(move || {
        reads.set(reads.get() + 1);
        Ok(format!("{}{path}", crate::contributions_support::origin()))
    })
}
/// Configured, changed, absent, frame-direct and failing addresses.
async fn addresses(page: &wasm_bindgen::JsValue, root: &DomRoot, artifact: &mut HtmlArtifact) {
    let view = view();
    let (first, second, direct) = (Rc::default(), Rc::default(), Rc::default());
    artifact.sandbox_url_provider = Some(address("/sandbox.html", &first));
    artifact.execute_content(LOGGING).unwrap();
    assert_eq!(
        frame(root).unwrap().src(),
        format!("{}/sandbox.html", crate::contributions_support::origin())
    );
    logged(page, root, artifact, &view.i18n, "[log] one").await;
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
    assert!(document.contains("window[\"third\"]") && !document.contains("window[\"first\"]"));
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
    page: &wasm_bindgen::JsValue,
    root: &DomRoot,
    artifact: &HtmlArtifact,
    reads: &Rc<Cell<u32>>,
) {
    let view = view();
    let adapter = artifact.sandbox_iframe_ref.borrow().clone().unwrap();
    let mut target = HtmlArtifact::new();
    target.filename = "target.html".to_owned();
    target.runtime_providers = vec![marker("target", reads)];
    target.set_content(LOGGING.to_owned()).unwrap();
    target.update(root, &view).unwrap();
    assert!(artifact.sandbox_iframe_ref.borrow().is_none());
    let moved = target.sandbox_iframe_ref.borrow().clone().unwrap();
    assert!(Rc::ptr_eq(&moved, &adapter));
    target.execute_content(target.content()).unwrap();
    let document = srcdoc(root);
    assert!(document.contains("\"artifact-target.html\"") && document.contains("runtime_target"));
    logged(page, root, &target, &view.i18n, "[log] one").await;
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
        _: maestro_app::browser::components::sandbox::sandbox_runtime_provider::SandboxRespond<
            BrowserSandboxValue,
        >,
    ) -> maestro_app::BoxFuture<Result<(), DiagnosticErrorInfo>> {
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
    fn get_runtime(&self) -> Result<String, DiagnosticErrorInfo> {
        Ok("() => {}".to_owned())
    }
    fn get_description(&self) -> Result<String, DiagnosticErrorInfo> {
        Ok(String::new())
    }
}
/// Message methods posted after the value domains, with the line kind each selects.
const METHODS: [(&str, &str); 12] = [
    ("'log'", "log"),
    ("'warn'", "log"),
    ("'info'", "log"),
    ("'debug'", "log"),
    ("'error'", "error"),
    ("undefined", "log"),
    ("null", "log"),
    ("42", "log"),
    ("true", "log"),
    ("{}", "log"),
    ("[]", "log"),
    ("'ERROR'", "log"),
];
/// A page posting raw messages to its parent: every recorded value domain, ignored message
/// types, every method kind, then the texts a provider edits in place.
fn messages_page(values: &str, cases: &serde_json::Value) -> String {
    let methods = METHODS.map(|(method, _)| method).join(",");
    format!(
        "<script type=\"module\">{values}
const send = message => window.parent.postMessage({{...message, sandboxId: window.sandboxId}}, '*');
for (const [kind, value] of {cases}) send(kind === 'missing' ? {{type: 'console'}} : {{type: 'console', text: operand(kind, value)}});
send({{text: 'ignored'}});
for (const type of [null, 'download', 'console-other', 17, false, {{}}, [], 'Console']) send({{type, text: 'ignored'}});
for (const method of [{methods}]) send(method === undefined ? {{type: 'console', text: 'line'}} : {{type: 'console', method, text: 'line'}});
send({{type: 'console', text: 'edit-me'}});
send({{type: 'console', text: 'delete-me'}});
send({{type: 'console', marker: 'add'}});
</script>"
    )
}
/// Only console messages append, keeping each raw text, the error kind only for `error`, and
/// the frame lives exactly as long as the connected root.
#[wasm_bindgen]
pub async fn html_artifact_dom_routes_logs_and_releases_its_frame(import_module: js_sys::Function) {
    let view = view();
    let _host = host(&view, &import_module);
    let page = page_module(&import_module).await;
    guidance(&view);
    let factory = crate::native_artifact_console_support::factory(&import_module).await;
    let values_url =
        wasm_bindgen::link_to!(module = "/tests/native_artifact_console_support/values.js");
    let values = crate::contributions_support::fetch_text(&values_url).await;
    let cases = crate::native_artifact_console_support::cases().values;
    let operands: Vec<_> = cases
        .iter()
        .map(|case| serde_json::json!([case.kind, case.operand]))
        .collect();
    let mut artifact = HtmlArtifact::new();
    artifact.filename = "routes.html".to_owned();
    artifact.runtime_providers.push(SharedHandle::new(Editing));
    artifact
        .set_content(messages_page(&values, &serde_json::Value::Array(operands)))
        .unwrap();
    let root = artifact.mount(&view).unwrap();
    let parent = container();
    parent.append_child(&root.node).unwrap();
    let (returned, painted) = expected_lines(&factory, &cases);
    logged(&page, &root, &artifact, &view.i18n, &returned.join("\n")).await;
    crate::native_html_artifact_support::expand(&root);
    assert_eq!(crate::native_html_artifact_support::painted(&root), painted);
    growth(&page, &root, &mut artifact).await;
    parent.remove();
    lifetime(&page).await;
}
/// The returned and painted lines the messages page produces, in order.
fn expected_lines(
    factory: &wasm_bindgen::JsValue,
    cases: &[crate::native_artifact_console_support::Value],
) -> (Vec<String>, Vec<String>) {
    let mut returned = Vec::new();
    let mut painted = Vec::new();
    for case in cases {
        let (copy, paint) = if case.kind == "date" {
            let operand = js_sys::JSON::parse(&case.operand.to_string()).unwrap();
            let args = js_sys::Array::of2(&case.kind.as_str().into(), &operand);
            let date = crate::native_artifact_console_support::native(factory, "operand", &args);
            let line = format!(
                "[log] {}",
                crate::native_artifact_console_support::native(
                    factory,
                    "nativeString",
                    &js_sys::Array::of1(&date)
                )
                .as_string()
                .unwrap()
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
/// Changed sources reset the logs, and 16, 32 and 64 messages arrive whole and in order.
async fn growth(page: &wasm_bindgen::JsValue, root: &DomRoot, artifact: &mut HtmlArtifact) {
    let view = view();
    for count in [16, 32, 64] {
        artifact
            .set_content(format!(
                "<script type=\"module\">for (let i = 0; i < {count}; i++) window.parent.postMessage({{type: 'console', text: 'line-' + i, sandboxId: window.sandboxId}}, '*');</script>"
            ))
            .unwrap();
        let expected: Vec<_> = (0..count)
            .map(|index| format!("[log] line-{index}"))
            .collect();
        logged(page, root, artifact, &view.i18n, &expected.join("\n")).await;
    }
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
async fn lifetime(page: &wasm_bindgen::JsValue) {
    let view = view();
    let parent = container();
    let mut first = HtmlArtifact::new();
    first.filename = "first.html".to_owned();
    first.set_content(LOGGING.to_owned()).unwrap();
    let mut sibling = HtmlArtifact::new();
    sibling.filename = "sibling.html".to_owned();
    sibling.set_content(LOGGING.to_owned()).unwrap();
    let root = first.mount(&view).unwrap();
    let sibling_root = sibling.mount(&view).unwrap();
    parent.append_child(&root.node).unwrap();
    parent.append_child(&sibling_root.node).unwrap();
    logged(page, &root, &first, &view.i18n, "[log] one").await;
    let adapter = first.sandbox_iframe_ref.borrow().clone().unwrap();
    parent.remove_child(&root.node).unwrap();
    assert!(first.sandbox_iframe_ref.borrow().is_none() && frame(&root).is_none());
    parent.append_child(&root.node).unwrap();
    let live = first.sandbox_iframe_ref.borrow().clone().unwrap();
    assert!(Rc::ptr_eq(&live, &adapter) && frame(&root).is_some());
    drop(live);
    parent.remove_child(&root.node).unwrap();
    drop(root);
    assert!(first.sandbox_iframe_ref.borrow().is_none());
    assert_eq!(
        Rc::strong_count(&adapter),
        1,
        "only the retained handle remains"
    );
    adapter.remove().unwrap();
    assert!(frame(&sibling_root).is_some());
    logged(page, &sibling_root, &sibling, &view.i18n, "[log] one").await;
    sibling.execute_content(sibling.content()).unwrap();
    logged(
        page,
        &sibling_root,
        &sibling,
        &view.i18n,
        "[log] one\n[log] one",
    )
    .await;
    parent.remove();
}
/// Interleaved bodies and copy controls keep their own operands, untaken operands never reach
/// a later default element, and releasing one body leaves the other usable.
#[wasm_bindgen]
pub async fn html_artifact_dom_nested_creation_keeps_controllers(import_module: js_sys::Function) {
    let view = view();
    let _host = host(&view, &import_module);
    let page = page_module(&import_module).await;
    let german = localized_view(&view, "de");
    let parent = container();
    let (reads_a, reads_b) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
    let mut first = HtmlArtifact::new();
    first.filename = "a.html".to_owned();
    first.runtime_providers.push(marker("a", &reads_a));
    first.set_content(LOGGING.to_owned()).unwrap();
    let root_a = first.mount(&view).unwrap();
    parent.append_child(&root_a.node).unwrap();
    logged(&page, &root_a, &first, &view.i18n, "[log] one").await;
    crate::native_html_artifact_support::expand(&root_a);
    let mut second = HtmlArtifact::new();
    second.filename = "b.html".to_owned();
    second.runtime_providers.push(marker("b", &reads_b));
    second.set_content(LOGGING.to_owned()).unwrap();
    let root_b = second.mount(&german).unwrap();
    parent.append_child(&root_b.node).unwrap();
    logged(&page, &root_b, &second, &german.i18n, "[log] one").await;
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
    untaken(&view, &parent);
    parent.remove_child(&root_a.node).unwrap();
    drop(root_a);
    assert!(first.sandbox_iframe_ref.borrow().is_none());
    second.execute_content(second.content()).unwrap();
    assert_eq!(reads_b.get(), 2);
    logged(
        &page,
        &root_b,
        &second,
        &german.i18n,
        "[log] one\n[log] one",
    )
    .await;
    parent.remove();
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
fn untaken(view: &DomView, parent: &Element) {
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
}
