//! Providers, sources and frame observations shared by the HTML artifact browser witnesses.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::contributions_support::{text, until};
use maestro_app::{DiagnosticErrorInfo, SandboxRuntimeProvider, SharedHandle};
use maestro_web::{
    I18n, browser::sandbox_value::BrowserSandboxValue, dom::DomRoot,
    tools::artifacts::html_artifact::HtmlArtifact,
};
use std::{cell::Cell, rc::Rc};
use wasm_bindgen::{JsCast as _, JsValue};
use web_sys::{Element, HtmlIFrameElement};

/// One runtime provider as artifacts hold it.
pub(crate) type Provider = SharedHandle<dyn SandboxRuntimeProvider<BrowserSandboxValue>>;
/// A source that writes one log line when it runs.
pub(crate) const LOGGING: &str =
    "<html><body>proof<script>console.log(\"one\")</script></body></html>";
/// A source that writes no log line.
pub(crate) const QUIET: &str = "<p>quiet</p>";

/// A provider counting its data reads, publishing `name` as a global and recording its own
/// runtime under that name, so a prepared document shows the provider order.
pub(crate) struct Marker {
    /// Name of the global and of the runtime marker.
    pub(crate) name: &'static str,
    /// Data reads so far.
    pub(crate) reads: Rc<Cell<u32>>,
    /// Whether data reads fail.
    pub(crate) fails: Rc<Cell<bool>>,
}
impl SandboxRuntimeProvider<BrowserSandboxValue> for Marker {
    fn get_data(
        &self,
    ) -> Result<indexmap::IndexMap<String, BrowserSandboxValue>, DiagnosticErrorInfo> {
        self.reads.set(self.reads.get() + 1);
        if self.fails.get() {
            return Err(DiagnosticErrorInfo::error("provider failed"));
        }
        Ok([(self.name.to_owned(), BrowserSandboxValue(self.name.into()))].into())
    }
    fn get_runtime(&self) -> Result<String, DiagnosticErrorInfo> {
        Ok(format!("() => {{ window.runtime_{} = true; }}", self.name))
    }
    fn get_description(&self) -> Result<String, DiagnosticErrorInfo> {
        Ok(String::new())
    }
}
/// A provider named `name` counting into `reads`.
pub(crate) fn marker(name: &'static str, reads: &Rc<Cell<u32>>) -> Provider {
    SharedHandle::new(Marker {
        name,
        reads: Rc::clone(reads),
        fails: Rc::default(),
    })
}
/// A provider whose data reads fail, counting into `reads`.
pub(crate) fn failing(reads: &Rc<Cell<u32>>) -> Provider {
    SharedHandle::new(Marker {
        name: "failing",
        reads: Rc::clone(reads),
        fails: Rc::new(Cell::new(true)),
    })
}
/// The body's isolated frame, when one is placed.
pub(crate) fn frame(root: &DomRoot) -> Option<HtmlIFrameElement> {
    root.node
        .unchecked_ref::<Element>()
        .query_selector(".maestro-html-preview iframe")
        .unwrap()
        .map(wasm_bindgen::JsCast::unchecked_into)
}
/// The frame's prepared document.
pub(crate) fn srcdoc(root: &DomRoot) -> String {
    frame(root).unwrap().srcdoc()
}
/// Whether `first` appears in `document` before `second`.
pub(crate) fn before(document: &str, first: &str, second: &str) -> bool {
    matches!((document.find(first), document.find(second)), (Some(a), Some(b)) if a < b)
}
/// Wait, by changes below `root`, until `artifact` reports `expected` logs.
pub(crate) async fn logged(
    page: &JsValue,
    root: &DomRoot,
    artifact: &HtmlArtifact,
    i18n: &I18n,
    expected: &str,
) {
    until(page, &root.node, || {
        artifact.get_logs(i18n).unwrap() == expected
    })
    .await;
}
/// The listed source text of a body.
pub(crate) fn listed(root: &DomRoot) -> String {
    text(&crate::contributions_support::select(
        &root.node,
        ".maestro-html-source code",
    ))
}
/// The painted console rows of a body, after expanding its console.
pub(crate) fn painted(root: &DomRoot) -> Vec<String> {
    let rows = crate::contributions_support::select(&root.node, ".maestro-artifact-logs");
    let children = rows.child_nodes();
    (0..children.length())
        .map(|index| {
            children
                .item(index)
                .unwrap()
                .unchecked_into::<web_sys::HtmlElement>()
                .inner_text()
        })
        .collect()
}
/// Activate the console summary of a body.
pub(crate) fn expand(root: &DomRoot) {
    crate::contributions_support::select(&root.node, ".maestro-artifact-summary")
        .unchecked_into::<web_sys::HtmlElement>()
        .click();
}
