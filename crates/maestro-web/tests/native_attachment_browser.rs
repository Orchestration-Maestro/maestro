//! Native attachment controls exercised through their registered page elements.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::contributions_support::{body, host, view};
use js_sys::{Array, Function, Reflect};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{JsCast as _, JsValue, closure::Closure, prelude::wasm_bindgen};
use web_sys::{CustomEvent, Element, HtmlElement};

/// Assign an unreflected element operand.
fn set(element: &Element, name: &str, value: &JsValue) {
    assert!(Reflect::set(element, &name.into(), value).unwrap());
}
/// Read the selected index from the actual element.
fn selected(element: &Element) -> JsValue {
    Reflect::get(element, &"selectedIndex".into()).unwrap()
}
/// Replace authored mode labels.
fn labels(element: &Element, names: &[&str]) {
    let values = names
        .iter()
        .map(|name| JsValue::from_str(name))
        .collect::<Array>();
    set(element, "modes", &values);
}
/// Click one native button.
fn click(element: &Element, index: u32) {
    element
        .query_selector_all("button")
        .unwrap()
        .item(index)
        .unwrap()
        .unchecked_into::<HtmlElement>()
        .click();
}
/// Fewer than two labels leave no visible controls or empty group chrome.
fn assert_empty(element: &Element) {
    assert_eq!(element.query_selector_all("button").unwrap().length(), 0);
    assert!(
        element
            .first_element_child()
            .unwrap()
            .get_bounding_client_rect()
            .width()
            .abs()
            < f64::EPSILON,
        "an empty mode group has no visible chrome"
    );
}
/// Authored label cardinality, literal text and unchanged button identity.
fn assert_labels(element: &Element) {
    labels(element, &[]);
    assert_empty(element);
    labels(element, &["Only"]);
    assert_empty(element);
    labels(element, &["File", "Text", "<&雪>"]);
    let buttons = element.query_selector_all("button").unwrap();
    assert_eq!(
        buttons.length(),
        3,
        "the registered element renders authored modes"
    );
    assert_eq!(buttons.item(2).unwrap().text_content().unwrap(), "<&雪>");
    set(element, "selectedIndex", &2.into());
    assert!(
        buttons
            .item(2)
            .unwrap()
            .unchecked_into::<Element>()
            .class_list()
            .contains("maestro-toggle-selected")
    );
    labels(element, &["File", "Text", "<&雪>"]);
    assert!(
        buttons.item(0).unwrap().is_same_node(
            element
                .query_selector("button")
                .unwrap()
                .as_ref()
                .map(AsRef::as_ref)
        )
    );
}
/// Record one changed selection and select a different mode during delivery.
fn recorder(
    element: &Element,
    seen: &Rc<RefCell<Vec<(f64, String)>>>,
) -> Closure<dyn Fn(CustomEvent)> {
    let (events, target) = (Rc::clone(seen), element.clone());
    Closure::<dyn Fn(CustomEvent)>::new(move |event: CustomEvent| {
        assert!(event.bubbles());
        assert_eq!(selected(&target), JsValue::from(1));
        let detail = event.detail();
        events.borrow_mut().push((
            Reflect::get(&detail, &"index".into())
                .unwrap()
                .as_f64()
                .unwrap(),
            Reflect::get(&detail, &"mode".into())
                .unwrap()
                .as_string()
                .unwrap(),
        ));
        set(&target, "selectedIndex", &0.into());
    })
}
/// Default selection and a nonzero selection retained through reconnection.
fn mounted(view: &maestro_web::dom::DomView) -> Element {
    let element = view.document.create_element("mode-toggle").unwrap();
    assert_eq!(selected(&element), JsValue::from(0));
    body().append_child(&element).unwrap();
    assert_eq!(element.query_selector_all("button").unwrap().length(), 2);
    element.remove();
    labels(&element, &["File", "Text", "Third"]);
    set(&element, "selectedIndex", &2.into());
    body().append_child(&element).unwrap();
    assert!(
        element
            .query_selector_all("button")
            .unwrap()
            .item(2)
            .unwrap()
            .unchecked_into::<Element>()
            .class_list()
            .contains("maestro-toggle-selected")
    );
    element
}
/// Native labels, selection validation and reentrant event delivery share one control owner.
#[wasm_bindgen]
pub fn native_mode_group_updates_labels_selection_and_reentrant_events(import_module: &Function) {
    let view = view();
    let _host = host(&view, import_module);
    let element = mounted(&view);
    assert_labels(&element);
    let seen = Rc::new(RefCell::new(Vec::new()));
    let listener = recorder(&element, &seen);
    element
        .add_event_listener_with_callback("mode-change", listener.as_ref().unchecked_ref())
        .unwrap();
    let choose = Reflect::get(&element, &"setMode".into())
        .unwrap()
        .unchecked_into::<Function>();
    for rejected in [-1, 3] {
        choose.call1(&element, &rejected.into()).unwrap();
        assert_eq!(selected(&element), JsValue::from(2));
    }
    click(&element, 2);
    assert!(seen.borrow().is_empty());
    click(&element, 1);
    assert_eq!(&*seen.borrow(), &[(1., "Text".to_owned())]);
    assert_eq!(selected(&element), JsValue::from(0));
    assert!(
        element
            .query_selector("button")
            .unwrap()
            .unwrap()
            .class_list()
            .contains("maestro-toggle-selected")
    );
    element
        .remove_event_listener_with_callback("mode-change", listener.as_ref().unchecked_ref())
        .unwrap();
    labels(&element, &["Replaced", "Second"]);
    assert_eq!(element.query_selector_all("button").unwrap().length(), 2);
    assert_eq!(
        element
            .query_selector("button")
            .unwrap()
            .unwrap()
            .text_content()
            .unwrap(),
        "Replaced"
    );
    element.remove();
}
