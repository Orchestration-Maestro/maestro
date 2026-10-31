//! Page operands, the fixture package and browser observations shared by contribution exports.
#![cfg(all(test, target_arch = "wasm32"))]
use maestro_app::{SandboxUrlProvider, SharedHandle, TextContent, ToolResultMessage, UserBlock};
use maestro_web::{
    I18n,
    contributions::{BrowserContribution, ContributionHost},
    dom::{DomContent, DomRoot, DomView},
    tools::types::ToolUiError,
};
use serde_json::Value;
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{JsCast as _, JsValue, closure::Closure};
use wasm_bindgen_futures::JsFuture;
use web_sys::{Document, Element, HtmlElement, Node};

/// Supplying package of every fixture contribution, as the resource loader names it.
pub(crate) const PACKAGE: &str = "./package";

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
        tool_renderers: Rc::default(),
    })
}
/// Replace only localization, keeping the supplied document, syntax engine and brand operands.
pub(crate) fn localized_view(view: &DomView, language: &str) -> Rc<DomView> {
    let i18n = I18n::new(Rc::new(crate::support::Host {
        stored: Some(language.to_owned()).into(),
        ..crate::support::Host::default()
    }))
    .unwrap();
    Rc::new(DomView {
        i18n: Rc::new(i18n),
        ..view.clone()
    })
}
/// The isolated-frame document served beside the page, which carries no page policy.
pub(crate) fn sandbox_url() -> SandboxUrlProvider {
    SharedHandle::new(|| Ok(format!("{}/sandbox.html", origin())))
}
/// A host for `view` that imports modules through the page's `import_module`.
pub(crate) fn host(view: &Rc<DomView>, import_module: &js_sys::Function) -> ContributionHost {
    ContributionHost::new(view, Some(sandbox_url()), import_module.clone()).unwrap()
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
/// The fixture package's contributions as plain JSON, which the resource loader's own output must equal.
pub(crate) async fn contributions() -> Vec<BrowserContribution> {
    serde_json::from_str(&fetch_text(&format!("{}contributions.json", base())).await).unwrap()
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
/// A meter reading the UTF-16 code units that changes below `target` published since its
/// previous reading, outside elements matching a nonempty `skip` selector.
pub(crate) async fn published(module: &JsValue, target: &Node, skip: &str) -> impl Fn() -> f64 {
    let measure = call(module, "written", &js_sys::Array::of1(&skip.into())).await;
    let take: js_sys::Function = call(module, "recorder", &js_sys::Array::of2(target, &measure))
        .await
        .dyn_into()
        .unwrap();
    move || take.call0(&JsValue::NULL).unwrap().as_f64().unwrap()
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
        usage: None,
    }
}
/// Page content showing fixed text in a paragraph.
pub(crate) struct Text(pub &'static str);
impl DomContent for Text {
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        let root = view.document.create_element("p").unwrap();
        root.set_text_content(Some(self.0));
        Ok(DomRoot::new(root))
    }
    fn update(&self, root: &DomRoot, _: &DomView) -> Result<(), ToolUiError> {
        root.node.set_text_content(Some(self.0));
        Ok(())
    }
}
/// A CSS `rgb(...)` value for a `#rrggbb` brand color.
pub(crate) fn rgb(hex: &str) -> String {
    let channel = |at: usize| u8::from_str_radix(&hex[at..at + 2], 16).unwrap();
    format!("rgb({}, {}, {})", channel(1), channel(3), channel(5))
}
/// One computed style property of an element.
pub(crate) fn computed(element: &Element, property: &str) -> String {
    web_sys::window()
        .unwrap()
        .get_computed_style(element)
        .unwrap()
        .unwrap()
        .get_property_value(property)
        .unwrap()
}

/// Outcome of a stubbed clipboard write.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Clipboard {
    /// The write succeeds at once.
    Accept,
    /// The page refuses the write.
    Refuse,
    /// The write succeeds when the test settles it.
    Defer,
}
/// A stubbed page clipboard recording each written text.
pub(crate) struct Recorded {
    /// Texts written so far.
    texts: Rc<RefCell<Vec<String>>>,
    /// Settlements of deferred writes, oldest last.
    settlements: Rc<RefCell<Vec<js_sys::Function>>>,
    /// The installed write, which stays reachable while this recorder lives.
    _write: Closure<dyn Fn(String) -> js_sys::Promise>,
}
impl Recorded {
    /// Replace the page clipboard with a recorder settling each write by `outcome`.
    pub(crate) fn install(outcome: Clipboard) -> Self {
        let texts: Rc<RefCell<Vec<String>>> = Rc::default();
        let settlements: Rc<RefCell<Vec<js_sys::Function>>> = Rc::default();
        let (recorded, deferred) = (Rc::clone(&texts), Rc::clone(&settlements));
        let write = Closure::<dyn Fn(String) -> js_sys::Promise>::new(move |text: String| {
            recorded.borrow_mut().push(text);
            match outcome {
                Clipboard::Accept => js_sys::Promise::resolve(&JsValue::UNDEFINED),
                Clipboard::Refuse => js_sys::Promise::reject(&JsValue::from_str("refused")),
                Clipboard::Defer => js_sys::Promise::new(&mut |resolve, _reject| {
                    deferred.borrow_mut().push(resolve);
                }),
            }
        });
        let navigator = web_sys::window().unwrap().navigator();
        let clipboard = js_sys::Reflect::get(&navigator, &"clipboard".into()).unwrap();
        js_sys::Reflect::set(&clipboard, &"writeText".into(), write.as_ref()).unwrap();
        Self {
            texts,
            settlements,
            _write: write,
        }
    }
    /// The texts written so far.
    pub(crate) fn texts(&self) -> Vec<String> {
        self.texts.borrow().clone()
    }
    /// Settle the oldest deferred write.
    pub(crate) fn settle(&self) {
        let next = self.settlements.borrow_mut().pop();
        if let Some(resolve) = next {
            resolve.call0(&JsValue::UNDEFINED).unwrap();
        }
    }
}
/// Wait for `count` animation frames.
pub(crate) async fn frames(page: &JsValue, count: u32) {
    call(page, "frames", &js_sys::Array::of1(&JsValue::from(count))).await;
}
/// The selected text and whether `root` holds the selection.
pub(crate) async fn selection(page: &JsValue, root: &Element) -> (String, bool) {
    let pair = js_sys::Array::from(&call(page, "selection", &js_sys::Array::of1(root)).await);
    (
        pair.get(0).as_string().unwrap(),
        pair.get(1).as_bool().unwrap(),
    )
}

/// One required descendant, reporting the missing selector.
pub(crate) fn select(node: &Node, selector: &str) -> Element {
    node.unchecked_ref::<Element>()
        .query_selector(selector)
        .unwrap()
        .unwrap_or_else(|| panic!("missing {selector}"))
}
/// Whether `element` holds the page focus.
pub(crate) fn focused(element: &Element) -> bool {
    document()
        .active_element()
        .is_some_and(|active| active.is_same_node(Some(element)))
}
/// The disclosure button of a collapsible tool header row, absent from a plain header.
pub(crate) fn disclosure(header: &Node) -> Option<HtmlElement> {
    header
        .unchecked_ref::<Element>()
        .query_selector(":scope > button.maestro-tool-chevron")
        .unwrap()
        .map(wasm_bindgen::JsCast::unchecked_into)
}
