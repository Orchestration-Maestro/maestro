//! HTML artifact bodies observed through the native document.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::contributions_support::{
    call, container, host, page_module, select, selection, text, view,
};
use maestro_app::{DiagnosticErrorInfo, SandboxRuntimeProvider, SharedHandle};
use maestro_web::{
    browser::sandbox_value::BrowserSandboxValue,
    dom::{DomContent as _, DomRoot, DomView},
    tools::artifacts::html_artifact::HtmlArtifact,
};
use std::rc::Rc;
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
/// Mount a named artifact, in preview with an empty listing, whose provider and sandbox address
/// fail the test when read.
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
    container().append_child(&root.node).unwrap();
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
    let listing = code(&root);
    let token = first_token(&root);
    call(&page, "select", &js_sys::Array::of1(&listing)).await;
    artifact.set_content(artifact.content().to_owned()).unwrap();
    artifact.filename = "renamed.html".to_owned();
    artifact.runtime_providers.push(SharedHandle::new(Unread));
    artifact.update(&root, &view).unwrap();
    assert!(code(&root).is_same_node(Some(&listing)));
    assert!(first_token(&root).is_same_node(Some(&token)));
    assert!(selection(&page, &listing).await.1);
    assert!(artifact.sandbox_iframe_ref.borrow().is_none());
    let node: &Element = root.node.unchecked_ref();
    assert!(node.query_selector("iframe").unwrap().is_none());
    artifact.set_content("<b>changed</b>".to_owned()).unwrap();
    artifact.update(&root, &view).unwrap();
    assert!(code(&root).is_same_node(Some(&listing)));
    assert_eq!(text(&code(&root)), "<b>changed</b>");
    assert!(code(&root).query_selector("b").unwrap().is_none());
}
