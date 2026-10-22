//! Page operands, the fixture package and browser observations shared by contribution exports.
#![cfg(all(test, target_arch = "wasm32"))]
use maestro_app::{SandboxUrlProvider, SharedHandle, TextContent, ToolResultMessage, UserBlock};
use maestro_web::{
    I18n,
    contributions::{BrowserContribution, ContributionHost},
    dom::{DomContent, DomView},
    tools::types::ToolUiError,
};
use serde_json::Value;
use std::rc::Rc;
use wasm_bindgen::{JsCast as _, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{Document, Element, HtmlElement, Node};

/// Supplying package of every fixture contribution.
pub(crate) const PACKAGE: &str = "fixture-package";

/// The page document.
pub(crate) fn document() -> Document {
    web_sys::window().unwrap().document().unwrap()
}
/// The page body.
pub(crate) fn body() -> HtmlElement {
    document().body().unwrap()
}
/// The page origin.
pub(crate) fn origin() -> String {
    web_sys::window().unwrap().location().origin().unwrap()
}
/// The package base URL the harness serves the fixture package from.
pub(crate) fn base() -> String {
    format!("{}/package/", origin())
}
/// Page operands with the shared brand pack and syntax engine; panics report their message.
pub(crate) fn view() -> Rc<DomView> {
    std::panic::set_hook(Box::new(|info| {
        web_sys::console::error_1(&info.to_string().into());
    }));
    Rc::new(DomView {
        document: document(),
        i18n: Rc::new(I18n::new(Rc::new(crate::support::Host::default())).unwrap()),
        highlighter: Rc::new(maestro_theme::SyntectHighlighter::new().unwrap()),
        pack: Rc::new(crate::brand::pack("brand.json").unwrap()),
        mode: maestro_theme::BrandMode::Dark,
    })
}
/// The isolated-frame document served beside the page, which carries no page policy.
pub(crate) fn sandbox_url() -> SandboxUrlProvider {
    SharedHandle::new(|| Ok(format!("{}/sandbox.html", origin())))
}
/// A host for `view` that imports modules through the page's `import_module`.
pub(crate) fn host(view: &Rc<DomView>, import_module: &js_sys::Function) -> ContributionHost {
    ContributionHost::new(Rc::clone(view), Some(sandbox_url()), import_module.clone()).unwrap()
}
/// Await a promise, or take a plain value as settled.
pub(crate) async fn settle(value: JsValue) -> Result<JsValue, JsValue> {
    JsFuture::from(js_sys::Promise::resolve(&value)).await
}
/// Fetch a text asset.
pub(crate) async fn fetch_text(url: &str) -> String {
    let window = web_sys::window().unwrap();
    let response: web_sys::Response = JsFuture::from(window.fetch_with_str(url))
        .await
        .unwrap()
        .dyn_into()
        .unwrap();
    assert!(response.ok(), "{url}: HTTP {}", response.status());
    JsFuture::from(response.text().unwrap())
        .await
        .unwrap()
        .as_string()
        .unwrap()
}
/// The fixture package's contributions, read from its manifest's `maestro.contributions` files.
pub(crate) async fn contributions() -> Vec<BrowserContribution> {
    let manifest: Value =
        serde_json::from_str(&fetch_text(&format!("{}package.json", base())).await).unwrap();
    let mut accepted = Vec::new();
    for file in manifest["maestro"]["contributions"].as_array().unwrap() {
        let text = fetch_text(&format!("{}{}", base(), file.as_str().unwrap())).await;
        accepted.push(BrowserContribution {
            package: PACKAGE.into(),
            descriptor: serde_json::from_str(&text).unwrap(),
        });
    }
    accepted
}
/// The browser observation helpers, imported through the page's own module import.
pub(crate) async fn page_module(import_module: &js_sys::Function) -> JsValue {
    let url = wasm_bindgen::link_to!(module = "/tests/contributions_support/page.js");
    settle(import_module.call1(&JsValue::NULL, &url.into()).unwrap())
        .await
        .unwrap()
}
/// Call a helper export and await its result.
pub(crate) async fn call(module: &JsValue, name: &str, args: &js_sys::Array) -> JsValue {
    let function: js_sys::Function = js_sys::Reflect::get(module, &name.into())
        .unwrap()
        .dyn_into()
        .unwrap();
    settle(function.apply(&JsValue::NULL, args).unwrap())
        .await
        .unwrap()
}
/// Wait, by mutation signals, until `ready` holds below `target`.
pub(crate) async fn until(module: &JsValue, target: &Node, ready: impl Fn() -> bool) {
    while !ready() {
        call(module, "changed", &js_sys::Array::of1(target)).await;
    }
}
/// Call a driver binding the harness exposes on the page.
pub(crate) async fn driver(name: &str, argument: &str) -> JsValue {
    let window = web_sys::window().unwrap();
    let function: js_sys::Function = js_sys::Reflect::get(&window, &name.into())
        .unwrap()
        .dyn_into()
        .unwrap();
    settle(function.call1(&window, &argument.into()).unwrap())
        .await
        .unwrap()
}
/// A connected `maestro-slot` element named `name` with `input`.
pub(crate) fn slot(name: &str, input: &Value) -> Element {
    let element = document().create_element("maestro-slot").unwrap();
    element.set_attribute("name", name).unwrap();
    set_input(&element, input);
    body().append_child(&element).unwrap();
    element
}
/// Replace an element's `maestroInput` property.
pub(crate) fn set_input(element: &Element, input: &Value) {
    let value = js_sys::JSON::parse(&input.to_string()).unwrap();
    js_sys::Reflect::set(element, &"maestroInput".into(), &value).unwrap();
}
/// Text below a node.
pub(crate) fn text(node: &Node) -> String {
    node.text_content().unwrap_or_default()
}
/// A new connected container.
pub(crate) fn container() -> Element {
    let element = document().create_element("section").unwrap();
    body().append_child(&element).unwrap();
    element
}
/// A tool result with one text block.
pub(crate) fn result(output: &str, is_error: bool) -> ToolResultMessage<Value> {
    ToolResultMessage {
        tool_call_id: "call".into(),
        tool_name: "tool".into(),
        timestamp: 1.0,
        content: vec![UserBlock::Text(TextContent::new(output.into()))],
        details: None,
        is_error,
    }
}
/// Page content showing fixed text in a paragraph.
pub(crate) struct Text(pub &'static str);
impl DomContent for Text {
    fn mount(&self, view: &DomView) -> Result<Node, ToolUiError> {
        let root = view.document.create_element("p").unwrap();
        root.set_text_content(Some(self.0));
        Ok(root.into())
    }
    fn update(&self, root: &Node, _: &DomView) -> Result<(), ToolUiError> {
        root.set_text_content(Some(self.0));
        Ok(())
    }
}
