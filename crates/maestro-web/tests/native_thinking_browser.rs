//! The reasoning disclosure mounted as native page content and as its element.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::contributions_support::{
    body, call, computed, container, document, driver, host, page_module, rgb, view,
};
use maestro_theme::BrandMode;
use maestro_web::{
    ThinkingBlock,
    dom::{DomView, MountedContent},
};
use std::fmt::Write as _;
use wasm_bindgen::{JsCast as _, JsValue, prelude::wasm_bindgen};
use web_sys::{Element, HtmlElement};

/// Reasoning with leading indentation, which only untrimmed content shows as a listing.
const INDENTED: &str = "    indented reasoning";

/// Reasoning disclosure with `content`.
fn thinking(content: &str, is_streaming: bool) -> ThinkingBlock {
    let mut block = ThinkingBlock::default();
    content.clone_into(&mut block.content);
    block.is_streaming = is_streaming;
    block
}
/// The mounted disclosure root.
fn root(shown: &MountedContent) -> Element {
    shown.root().cloned().unwrap().unchecked_into()
}
/// The disclosure's native toggle.
fn header(root: &Element) -> HtmlElement {
    root.query_selector("button")
        .unwrap()
        .unwrap()
        .unchecked_into()
}
/// The shown reasoning body, if expanded.
fn reasoning(root: &Element) -> Option<Element> {
    root.query_selector(".maestro-markdown").unwrap()
}
/// The pack's color for `role` in `mode`, as computed CSS spells it.
fn color(view: &DomView, mode: BrandMode, role: &str) -> String {
    rgb(view.pack.presentation(mode).unwrap().colors[role])
}
/// Move the real pointer to `point`, clicking there when `click` is set.
async fn pointer(point: (f64, f64), click: bool) {
    driver(
        "driverPointer",
        &format!("[{},{},{click}]", point.0, point.1),
    )
    .await;
}
/// The header label paints at the small text size: as wide as a copy of it set at 0.875rem.
fn assert_small_header_text(toggle: &HtmlElement) {
    let label = toggle.last_element_child().unwrap();
    let probe: HtmlElement = label.clone_node_with_deep(true).unwrap().unchecked_into();
    probe.style().set_property("font-size", "0.875rem").unwrap();
    toggle.append_child(&probe).unwrap();
    let (shown, small) = (
        label.get_bounding_client_rect().width(),
        probe.get_bounding_client_rect().width(),
    );
    probe.remove();
    assert!(
        shown > 0. && (shown - small).abs() < 0.01,
        "header text {shown}px wide, small text {small}px"
    );
}
/// The center of `element` in page coordinates.
fn center(element: &Element) -> (f64, f64) {
    let rect = element.get_bounding_client_rect();
    (rect.x() + rect.width() / 2., rect.y() + rect.height() / 2.)
}

/// The literal header toggles a body that exists only while expanded and shows the latest source.
#[wasm_bindgen]
pub fn thinking_dom_disclosure_retains_state_and_exact_header(import_module: &js_sys::Function) {
    let view = view();
    let _host = host(&view, import_module);
    let block = ThinkingBlock::default();
    assert!(block.content.is_empty() && !block.is_streaming);
    let parent = container();
    let mut shown = MountedContent::default();
    shown
        .show(&thinking(INDENTED, false), &parent, &view)
        .unwrap();
    let mounted = root(&shown);
    let toggle = header(&mounted);
    assert_eq!(toggle.text_content().unwrap(), "Thinking...");
    assert_small_header_text(&toggle);
    assert_eq!(
        toggle.get_attribute("aria-expanded").as_deref(),
        Some("false")
    );
    assert!(reasoning(&mounted).is_none());
    toggle.click();
    let body = reasoning(&mounted).unwrap();
    assert_eq!(body.class_name(), "maestro-markdown maestro-thinking");
    let copy = body.query_selector("copy-button").unwrap().unwrap();
    assert_eq!(
        copy.get_attribute("text").as_deref(),
        Some("indented reasoning\n")
    );
    assert_eq!(
        toggle.get_attribute("aria-expanded").as_deref(),
        Some("true")
    );
    toggle.click();
    assert!(reasoning(&mounted).is_none());
    toggle.click();
    assert!(reasoning(&mounted).is_some());
    shown
        .show(&thinking("Second *step*", false), &parent, &view)
        .unwrap();
    assert_eq!(root(&shown), mounted);
    assert_eq!(
        reasoning(&mounted).unwrap().text_content().unwrap(),
        "Second step\n"
    );
    toggle.click();
    shown
        .show(&thinking("Third step", false), &parent, &view)
        .unwrap();
    assert!(reasoning(&mounted).is_none());
    toggle.click();
    assert_eq!(
        reasoning(&mounted).unwrap().text_content().unwrap(),
        "Third step\n"
    );
    assert_eq!(toggle.text_content().unwrap(), "Thinking...");
}

/// The header shimmers only while streaming, and instances keep separate expansion and content.
#[wasm_bindgen]
pub fn thinking_dom_streaming_and_instances_are_independent(import_module: &js_sys::Function) {
    let view = view();
    let _host = host(&view, import_module);
    let (first_parent, second_parent) = (container(), container());
    let (mut first, mut second) = (MountedContent::default(), MountedContent::default());
    first
        .show(&thinking("first", false), &first_parent, &view)
        .unwrap();
    second
        .show(&thinking("second", false), &second_parent, &view)
        .unwrap();
    let label = header(&root(&first)).last_element_child().unwrap();
    assert_eq!(computed(&label, "animation-name"), "none");
    for (streaming, animation) in [(true, "maestro-shimmer"), (false, "none")] {
        first
            .show(&thinking("first", streaming), &first_parent, &view)
            .unwrap();
        assert_eq!(computed(&label, "animation-name"), animation);
        let other = header(&root(&second)).last_element_child().unwrap();
        assert_eq!(computed(&other, "animation-name"), "none");
    }
    header(&root(&first)).click();
    assert!(reasoning(&root(&first)).is_some());
    assert!(reasoning(&root(&second)).is_none());
    second
        .show(&thinking("changed", true), &second_parent, &view)
        .unwrap();
    header(&root(&second)).click();
    assert_eq!(
        reasoning(&root(&first)).unwrap().text_content().unwrap(),
        "first\n"
    );
    assert_eq!(
        reasoning(&root(&second)).unwrap().text_content().unwrap(),
        "changed\n"
    );
    assert_eq!(computed(&label, "animation-name"), "none");
}

/// Pointer and keyboard toggle once; reconnection keeps one binding and released roots stay inert.
#[wasm_bindgen]
pub async fn thinking_dom_reconnect_and_drop_release_listeners(import_module: js_sys::Function) {
    let view = view();
    let _host = host(&view, &import_module);
    let parent = container();
    let mut shown = MountedContent::default();
    shown
        .show(&thinking("keyboard", false), &parent, &view)
        .unwrap();
    let after = document().create_element("button").unwrap();
    after.set_text_content(Some("after"));
    parent.append_child(&after).unwrap();
    let mounted = root(&shown);
    let toggle = header(&mounted);
    let tree = driver("driverAria", "section").await.as_string().unwrap();
    assert!(tree.contains("button \"Thinking...\""), "{tree}");
    driver("driverPress", "Tab").await;
    assert_eq!(document().active_element(), Some(toggle.clone().into()));
    for (key, expanded) in [("Enter", true), ("Space", false)] {
        driver("driverPress", key).await;
        assert_eq!(reasoning(&mounted).is_some(), expanded, "{key}");
    }
    pointer(center(&toggle), true).await;
    assert!(reasoning(&mounted).is_some());
    driver("driverPress", "Tab").await;
    assert_eq!(document().active_element(), Some(after));
    element_reconnects_with_one_binding();
    drop(shown);
    toggle.click();
    assert!(
        reasoning(&mounted).is_some(),
        "a released root receives no callback"
    );
    assert_eq!(
        toggle.get_attribute("aria-expanded").as_deref(),
        Some("true")
    );
}
/// A detached element keeps its expansion without a binding and reconnects with exactly one.
fn element_reconnects_with_one_binding() {
    let element = document().create_element("thinking-block").unwrap();
    js_sys::Reflect::set(&element, &"content".into(), &"element body".into()).unwrap();
    body().append_child(&element).unwrap();
    let inner = header(&element);
    inner.click();
    assert!(reasoning(&element).is_some());
    element.remove();
    inner.click();
    assert!(
        reasoning(&element).is_some(),
        "a detached element keeps no binding"
    );
    body().append_child(&element).unwrap();
    assert!(
        reasoning(&element).is_some(),
        "reconnection keeps expansion"
    );
    inner.click();
    assert!(reasoning(&element).is_none(), "one binding toggles once");
    let fresh = document().create_element("thinking-block").unwrap();
    body().append_child(&fresh).unwrap();
    assert!(reasoning(&fresh).is_none());
}

/// Header, chevron and shimmer paint the selected brand roles, with interruptible motion.
#[wasm_bindgen]
pub async fn thinking_dom_paints_brand_roles_and_disclosure_motion(
    import_module: js_sys::Function,
) {
    let page = page_module(&import_module).await;
    let view = view();
    let _host = host(&view, &import_module);
    for mode in [BrandMode::Dark, BrandMode::Light] {
        let parent = container();
        let mut properties = String::new();
        for (key, value) in view.pack.presentation(mode).unwrap().css_properties() {
            write!(properties, "{key}:{value};").unwrap();
        }
        parent.set_attribute("style", &properties).unwrap();
        let mut shown = MountedContent::default();
        shown
            .show(&thinking("body", false), &parent, &view)
            .unwrap();
        let toggle = header(&root(&shown));
        let roles = (color(&view, mode, "muted"), color(&view, mode, "text"));
        hover_moves_between_roles(&page, &toggle, &roles).await;
        chevron_turns_and_reverses(&page, &toggle).await;
        shown.show(&thinking("body", true), &parent, &view).unwrap();
        let label = toggle.last_element_child().unwrap();
        assert_eq!(
            (
                computed(&label, "animation-name"),
                computed(&label, "animation-duration")
            ),
            ("maestro-shimmer".to_owned(), "2s".to_owned())
        );
        let gradient = computed(&label, "background-image");
        assert!(
            gradient.contains(&roles.0) && gradient.contains(&roles.1),
            "{gradient}"
        );
        assert_eq!(computed(&label, "color"), "rgba(0, 0, 0, 0)");
        parent.remove();
    }
}
/// The arguments pausing `property` of `element` midway through its 150 ms transition.
fn midway(element: &Element, property: &str) -> js_sys::Array {
    js_sys::Array::of3(element, &property.into(), &JsValue::from(75))
}
/// Hovering moves the header and glyph from the muted to the text role through intermediate colors.
async fn hover_moves_between_roles(
    page: &JsValue,
    toggle: &HtmlElement,
    (muted, text): &(String, String),
) {
    let svg = toggle
        .first_element_child()
        .unwrap()
        .first_element_child()
        .unwrap();
    assert_eq!(&computed(toggle, "color"), muted);
    assert_eq!(&computed(&svg, "stroke"), muted);
    pointer(center(toggle), false).await;
    let duration = call(page, "seek", &midway(toggle, "color")).await;
    assert_eq!(duration.as_f64(), Some(150.));
    let between = computed(toggle, "color");
    assert!(&between != muted && &between != text, "{between}");
    call(page, "finish", &js_sys::Array::of1(toggle)).await;
    assert_eq!(&computed(toggle, "color"), text);
    assert_eq!(&computed(&svg, "stroke"), text);
    pointer((0., 0.), false).await;
    call(page, "finish", &js_sys::Array::of1(toggle)).await;
    assert_eq!(&computed(toggle, "color"), muted);
}
/// Expanding turns the chevron through intermediate angles; collapsing midway reverses it.
async fn chevron_turns_and_reverses(page: &JsValue, toggle: &HtmlElement) {
    let chevron = toggle.first_element_child().unwrap();
    assert_eq!(computed(&chevron, "transform"), "none");
    toggle.click();
    call(page, "seek", &midway(&chevron, "transform")).await;
    let turning = computed(&chevron, "transform");
    assert!(
        turning.starts_with("matrix(") && !turning.contains(", 1, -1,"),
        "{turning}"
    );
    toggle.click();
    call(page, "finish", &js_sys::Array::of1(&chevron)).await;
    assert_eq!(computed(&chevron, "transform"), "none");
    toggle.click();
    call(page, "finish", &js_sys::Array::of1(&chevron)).await;
    assert!(computed(&chevron, "transform").contains(", 1, -1,"));
    toggle.click();
}
