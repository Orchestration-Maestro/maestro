//! Fixture module A: a tool renderer, a message renderer and child tabs.
//!
//! Built separately from the host with its own binding glue, it registers the
//! contributions its package declares through the host interface it receives.
use custom_element::CustomElement;
use std::cell::{Cell, RefCell};
use wasm_bindgen::{JsCast as _, JsValue, closure::Closure, prelude::wasm_bindgen};
use web_sys::HtmlElement;

/// Constructor callbacks kept for the page lifetime.
type Classes = Vec<Closure<dyn FnMut(JsValue, js_sys::Array) -> JsValue>>;
thread_local! {
    /// Host interface received by the latest registration.
    static HOST: RefCell<JsValue> = const { RefCell::new(JsValue::UNDEFINED) };
    /// Constructor callbacks kept for the page lifetime.
    static CLASSES: RefCell<Classes> = const { RefCell::new(Vec::new()) };
    /// Whether a tab already registered the late contribution.
    static LATE: Cell<bool> = const { Cell::new(false) };
}
/// Read a dotted path of the element's input.
fn input(root: &HtmlElement, path: &[&str]) -> String {
    let mut value =
        js_sys::Reflect::get(root, &"maestroInput".into()).unwrap_or(JsValue::UNDEFINED);
    for key in path {
        value = js_sys::Reflect::get(&value, &(*key).into()).unwrap_or(JsValue::UNDEFINED);
    }
    value
        .as_string()
        .or_else(|| value.as_bool().map(|flag| flag.to_string()))
        .unwrap_or_default()
}
/// Call a host interface method with two arguments.
fn call(method: &str, first: &JsValue, second: &JsValue) -> Result<JsValue, JsValue> {
    HOST.with(|host| {
        let host = host.borrow().clone();
        let function: js_sys::Function = js_sys::Reflect::get(&host, &method.into())?.dyn_into()?;
        function.call2(&host, first, second)
    })
}
/// Call the provided greeting API through the host.
fn greeting() -> String {
    call("api", &"fixture.greeting".into(), &JsValue::UNDEFINED)
        .ok()
        .and_then(|api| api.dyn_into::<js_sys::Function>().ok())
        .and_then(|api| api.call1(&JsValue::NULL, &"A".into()).ok())
        .and_then(|text| text.as_string())
        .unwrap_or_else(|| "no greeting".into())
}
/// One view whose text is computed from its input.
struct View {
    /// Element, held weakly.
    root: js_sys::WeakRef,
    /// Text producer.
    text: fn(&HtmlElement) -> String,
    /// Input-change listener while connected.
    listener: Option<Closure<dyn Fn()>>,
    /// Whether the first connection registers the late contribution.
    late: bool,
}
impl View {
    /// The element, while it exists.
    fn element(&self) -> Option<HtmlElement> {
        self.root.deref().and_then(|root| root.dyn_into().ok())
    }
}
impl CustomElement for View {
    fn connected_callback(&mut self) {
        let Some(root) = self.element() else { return };
        root.set_text_content(Some(&(self.text)(&root)));
        let (weak, text) = (self.root.clone(), self.text);
        let listener = Closure::<dyn Fn()>::new(move || {
            let Some(root) = weak.deref().and_then(|r| r.dyn_into::<HtmlElement>().ok()) else {
                return;
            };
            let updates = root
                .get_attribute("data-updates")
                .and_then(|n| n.parse::<u32>().ok())
                .unwrap_or(0);
            let _ = root.set_attribute("data-updates", &(updates + 1).to_string());
            root.set_text_content(Some(&text(&root)));
            // An update handler may register too, including while the host releases this module.
            if input(&root, &["tab"]) == "again" {
                let _ = call("register", &"fixture-tool".into(), &"qa-tool-view".into());
            }
        });
        let _ = root
            .add_event_listener_with_callback("maestro-input", listener.as_ref().unchecked_ref());
        self.listener = Some(listener);
        if self.late && !LATE.get() {
            LATE.set(true);
            // Registering from inside a lifecycle callback exercises host reentrancy.
            let _ = call("register", &"fixture-late".into(), &"qa-tab-view".into());
        }
    }
    fn disconnected_callback(&mut self) {
        if let (Some(root), Some(listener)) = (self.element(), self.listener.take()) {
            let _ = root.remove_event_listener_with_callback(
                "maestro-input",
                listener.as_ref().unchecked_ref(),
            );
        }
    }
}
/// The tool view's text: parameters, streaming flag and the greeting API's answer.
fn tool_text(root: &HtmlElement) -> String {
    format!(
        "tool:{}:{}:{}",
        input(root, &["params", "text"]),
        input(root, &["isStreaming"]),
        greeting()
    )
}
/// The message view's text: the original role and content.
fn message_text(root: &HtmlElement) -> String {
    format!(
        "message:{}:{}",
        input(root, &["message", "role"]),
        input(root, &["message", "content"])
    )
}
/// The tab view's text.
fn tab_text(root: &HtmlElement) -> String {
    format!("tab:{}", input(root, &["tab"]))
}
/// Define one view class unless this page already defined it.
fn define(tag: &str, text: fn(&HtmlElement) -> String, late: bool) -> Result<(), JsValue> {
    let registry = web_sys::window().ok_or("no window")?.custom_elements();
    if !registry.get(tag).is_undefined() {
        return Ok(());
    }
    let (closure, constructor) = custom_element::create_custom_element(
        move |instance, _| View {
            root: js_sys::WeakRef::new(instance.unchecked_ref::<js_sys::Object>()),
            text,
            listener: None,
            late,
        },
        vec![],
    );
    registry.define(tag, &constructor)?;
    CLASSES.with(|all| all.borrow_mut().push(closure));
    Ok(())
}
/// Register the tool view again under a fresh element name, as a changed definition must.
/// # Errors
/// Returns a definition or registration failure.
#[wasm_bindgen]
pub fn upgrade() -> Result<(), JsValue> {
    define("qa-tool-view-2", tool_text, false)?;
    call(
        "register",
        &"fixture-tool".into(),
        &"qa-tool-view-2".into(),
    )
    .map(drop)
}
/// Registration export.
/// # Errors
/// Returns a definition or registration failure.
#[wasm_bindgen]
pub fn register(host: JsValue) -> Result<(), JsValue> {
    HOST.with(|current| *current.borrow_mut() = host);
    define("qa-tool-view", tool_text, false)?;
    define("qa-message-view", message_text, false)?;
    define("qa-tab-view", tab_text, true)?;
    for (id, tag) in [
        ("fixture-tool", "qa-tool-view"),
        ("fixture-message", "qa-message-view"),
        ("fixture-tab", "qa-tab-view"),
        ("fixture-bad-tab", "qa-tab-view"),
    ] {
        call("register", &id.into(), &tag.into())?;
    }
    Ok(())
}
