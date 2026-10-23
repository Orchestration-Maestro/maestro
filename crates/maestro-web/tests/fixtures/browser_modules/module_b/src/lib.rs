//! Fixture module B: a host API, a page with a child slot and Home cards.
//!
//! Built separately from the host and from module A, with its own binding glue.
//! It defines its element names without a guard, so a second registration in the
//! same page meets the browser's refusal to redefine a name.
use custom_element::CustomElement;
use std::cell::RefCell;
use wasm_bindgen::{JsCast as _, JsValue, closure::Closure, prelude::wasm_bindgen};
use web_sys::HtmlElement;

/// Constructor callbacks kept for the page lifetime.
type Classes = Vec<Closure<dyn FnMut(JsValue, js_sys::Array) -> JsValue>>;
/// Provided API functions kept for the page lifetime.
type Apis = Vec<Closure<dyn Fn(String) -> String>>;
thread_local! {
    /// Constructor callbacks and provided API functions kept for the page lifetime.
    static KEPT: RefCell<(Classes, Apis)> =
        const { RefCell::new((Vec::new(), Vec::new())) };
}
/// Page or card view.
struct View {
    /// Element, held weakly.
    root: js_sys::WeakRef,
    /// Whether this is the page, which supplies a child slot.
    page: bool,
}
impl CustomElement for View {
    fn connected_callback(&mut self) {
        let Some(root) = self
            .root
            .deref()
            .and_then(|r| r.dyn_into::<HtmlElement>().ok())
        else {
            return;
        };
        if root.child_element_count() > 0 {
            return;
        }
        let input =
            js_sys::Reflect::get(&root, &"maestroInput".into()).unwrap_or(JsValue::UNDEFINED);
        let title = js_sys::Reflect::get(&input, &"title".into())
            .ok()
            .and_then(|v| v.as_string())
            .unwrap_or_default();
        let kind = if self.page { "page" } else { "card" };
        let Some(document) = root.owner_document() else {
            return;
        };
        let Ok(heading) = document.create_element("span") else {
            return;
        };
        heading.set_text_content(Some(&format!("{kind}:{title}")));
        let _ = root.append_child(&heading);
        if self.page
            && let Ok(slot) = document.create_element("maestro-slot")
        {
            let _ = slot.set_attribute("name", "fixture-page.tabs");
            let tab = js_sys::Object::new();
            let _ = js_sys::Reflect::set(&tab, &"tab".into(), &"first".into());
            let _ = js_sys::Reflect::set(&slot, &"maestroInput".into(), &tab);
            let _ = root.append_child(&slot);
        }
    }
}
/// Define one view class.
fn define(tag: &str, page: bool) -> Result<(), JsValue> {
    let (closure, constructor) = custom_element::create_custom_element(
        move |instance, _| View {
            root: js_sys::WeakRef::new(instance.unchecked_ref::<js_sys::Object>()),
            page,
        },
        vec![],
    );
    web_sys::window()
        .ok_or("no window")?
        .custom_elements()
        .define(tag, &constructor)?;
    KEPT.with(|kept| kept.borrow_mut().0.push(closure));
    Ok(())
}
/// Record a refused host call's message for the page to read.
fn record(name: &str, outcome: Result<JsValue, JsValue>) {
    let text = outcome
        .err()
        .and_then(|error| error.as_string())
        .unwrap_or_default();
    if let Some(window) = web_sys::window() {
        let _ = js_sys::Reflect::set(&window, &name.into(), &text.into());
    }
}
/// Registration export.
/// # Errors
/// Returns a definition or registration failure.
#[wasm_bindgen]
pub fn register(host: JsValue) -> Result<(), JsValue> {
    let call = |method: &str, first: &JsValue, second: &JsValue| -> Result<JsValue, JsValue> {
        let function: js_sys::Function = js_sys::Reflect::get(&host, &method.into())?.dyn_into()?;
        function.call2(&host, first, second)
    };
    let greeting = Closure::<dyn Fn(String) -> String>::new(|who: String| format!("hello {who}"));
    call("provide", &"fixture-greeting".into(), greeting.as_ref())?;
    let second = Closure::<dyn Fn(String) -> String>::new(|who: String| format!("again {who}"));
    record(
        "qaDuplicateApi",
        call("provide", &"fixture-greeting".into(), second.as_ref()),
    );
    KEPT.with(|kept| kept.borrow_mut().1.extend([greeting, second]));
    record(
        "qaUndeclared",
        call(
            "register",
            &"fixture-undeclared".into(),
            &"qa-card-view".into(),
        ),
    );
    let number = JsValue::from_f64(7.0);
    record(
        "qaNonStringId",
        call("register", &number, &"qa-card-view".into()),
    );
    record(
        "qaNonStringTag",
        call("register", &"fixture-card".into(), &number),
    );
    record("qaNonStringApi", call("api", &number, &JsValue::UNDEFINED));
    define("qa-page-view", true)?;
    define("qa-card-view", false)?;
    for (id, tag) in [
        ("fixture-page", "qa-page-view"),
        ("fixture-card", "qa-card-view"),
        ("fixture-needs-missing", "qa-card-view"),
    ] {
        call("register", &id.into(), &tag.into())?;
    }
    Ok(())
}
