//! Native rendering and keyboard observations of the complete visible-state inventory.
use super::{buttons, provider};
use crate::contributions_support::{
    call, computed, container, driver, host, page_module, rgb, text, view,
};
use maestro_theme::{BrandMode, BrandPresentation};
use maestro_web::{
    CustomProviderCard,
    components::custom_provider_card::ProviderStatus,
    dom::{DomContent, DomView},
};
use std::{cell::Cell, rc::Rc};
use wasm_bindgen::{JsCast as _, JsValue};
use web_sys::{Element, HtmlElement};

/// Exercise both packs, both modes and the independent native branches.
pub(super) async fn check(import: js_sys::Function) {
    let view = view();
    let _host = host(&view, &import);
    let page = page_module(&import).await;
    for pack in ["brand.json", "signal.json"] {
        let pack = crate::brand::pack(pack).unwrap();
        for mode in [BrandMode::Dark, BrandMode::Light] {
            paint_sizes(&view, &pack.presentation(mode).unwrap());
        }
    }
    keyboard_card(&view, &page).await;
    offscreen_text(&view, &page).await;
}
/// Native widths and zoom retain the authored layout without a clipping surface.
fn paint_sizes(view: &DomView, presentation: &BrandPresentation<'_>) {
    for (width, zoom) in [(280, "1"), (800, "1.5")] {
        let parent = container();
        let style = parent.unchecked_ref::<HtmlElement>().style();
        for (key, value) in presentation.css_properties() {
            style.set_property(&key, &value).unwrap();
        }
        style.set_property("width", &format!("{width}px")).unwrap();
        style.set_property("zoom", zoom).unwrap();
        for state in [
            None,
            Some(ProviderStatus::Connected { model_count: 0 }),
            Some(ProviderStatus::Checking),
            Some(ProviderStatus::Disconnected),
        ] {
            paint_states(&parent, view, presentation, state);
        }
        parent.remove();
    }
}
/// Manual and discovery states remain independent of URLs and action presence.
fn paint_states(
    parent: &Element,
    view: &DomView,
    presentation: &BrandPresentation<'_>,
    state: Option<ProviderStatus>,
) {
    for discovery in [false, true] {
        for url in ["", "https://a.test"] {
            for actions in [0, 1, 3] {
                let mut card = CustomProviderCard::new(provider());
                card.provider.borrow_mut().name = "Named 雪".into();
                card.provider.borrow_mut().base_url = url.into();
                card.is_auto_discovery = discovery;
                card.status = state;
                set_actions(&mut card, actions);
                paint_instance(parent, view, presentation, &card);
            }
        }
    }
}
/// Populate the selected independent action count.
fn set_actions(card: &mut CustomProviderCard, count: usize) {
    if count > 0 {
        card.on_edit = Some(Rc::new(|_| {}));
    }
    if count == 3 {
        card.on_delete = Some(Rc::new(|_| {}));
        card.on_refresh = Some(Rc::new(|_| {}));
    }
}
/// Measure native text and controls, rather than reading authored declarations.
fn painted_parts(root: &Element, presentation: &BrandPresentation<'_>) {
    let name = root
        .query_selector(".maestro-provider-name")
        .unwrap()
        .unwrap();
    let details = root
        .query_selector(".maestro-provider-details")
        .unwrap()
        .unwrap();
    assert_eq!(
        computed(root, "border-top-color"),
        rgb(presentation.colors["border"])
    );
    assert_eq!(computed(root, "background-color"), "rgba(0, 0, 0, 0)");
    assert_eq!(computed(&name, "color"), rgb(presentation.colors["text"]));
    assert_eq!(
        computed(&details, "color"),
        rgb(presentation.colors["muted"])
    );
    assert!(name.get_bounding_client_rect().height() > 0.0);
    assert!(details.get_bounding_client_rect().top() >= name.get_bounding_client_rect().bottom());
    let controls = buttons(root);
    for button in &controls {
        assert_eq!(computed(button, "color"), rgb(presentation.colors["text"]));
        assert!(
            button.client_width() >= button.scroll_width(),
            "action label must not clip"
        );
        assert!(button.get_bounding_client_rect().height() > 0.0);
    }
    for pair in controls.windows(2) {
        assert!(
            pair[1].get_bounding_client_rect().left() > pair[0].get_bounding_client_rect().right()
        );
    }
}
/// Each rendered status dot uses its selected semantic role and has native glyph geometry.
fn painted_dot(
    root: &Element,
    presentation: &BrandPresentation<'_>,
    state: Option<ProviderStatus>,
) {
    let Some(state) = state else {
        assert!(
            root.query_selector(".maestro-provider-status")
                .unwrap()
                .is_none()
        );
        return;
    };
    let dot = root.query_selector(".maestro-provider-dot").unwrap();
    assert!(dot.is_some(), "discovery status must paint its own dot");
    let dot = dot.unwrap();
    let role = match state {
        ProviderStatus::Connected { .. } => "success",
        ProviderStatus::Checking => "warning",
        ProviderStatus::Disconnected => "error",
    };
    assert_eq!(text(&dot), "● ");
    assert_eq!(computed(&dot, "color"), rgb(presentation.colors[role]));
    assert!(dot.get_bounding_client_rect().width() > 0.0);
    let row = dot.parent_element().unwrap();
    let label = dot.next_element_sibling().unwrap();
    let scale = root.get_bounding_client_rect().width()
        / f64::from(root.unchecked_ref::<HtmlElement>().offset_width());
    let gap = label.get_bounding_client_rect().left() - dot.get_bounding_client_rect().right();
    assert_eq!(computed(&row, "display"), "flex");
    assert_eq!(computed(&row, "column-gap"), "4px");
    // Zoomed bounding rectangles carry native floating-point rounding.
    assert!((gap - 4.0 * scale).abs() < 0.03);
}
/// Offscreen caller text stays native, findable and selectable.
async fn offscreen_text(view: &DomView, page: &JsValue) {
    let card = CustomProviderCard::new(provider());
    card.provider.borrow_mut().base_url = "https://offscreen.test".into();
    let root = card.mount(view).unwrap();
    let parent = container();
    parent
        .unchecked_ref::<HtmlElement>()
        .style()
        .set_property("margin-top", "2000px")
        .unwrap();
    parent.append_child(&root.node).unwrap();
    assert!(parent.get_bounding_client_rect().top() > 600.0);
    for literal in ["<b>雪</b>", "https://offscreen.test", "Models: 0"] {
        assert_eq!(
            call(page, "find", &js_sys::Array::of1(&literal.into()))
                .await
                .as_bool(),
            Some(true)
        );
    }
    assert_eq!(
        call(page, "select", &js_sys::Array::of1(&root.node))
            .await
            .as_string()
            .unwrap(),
        "<b>雪</b>\nOllama • https://offscreen.test\nModels: 0"
    );
    parent.remove();
}
/// Tab and Shift-Tab traverse native actions once; Enter invokes exactly one action.
async fn keyboard_card(view: &Rc<DomView>, page: &JsValue) {
    let mut card = CustomProviderCard::new(provider());
    let count = Rc::new(Cell::new(0));
    let recorded = Rc::clone(&count);
    card.is_auto_discovery = true;
    card.on_refresh = Some(Rc::new(move |_| recorded.set(recorded.get() + 1)));
    card.on_edit = Some(Rc::new(|_| {}));
    card.on_delete = Some(Rc::new(|_| {}));
    let parent = container();
    let before: HtmlElement = view
        .document
        .create_element("button")
        .unwrap()
        .unchecked_into();
    let after: HtmlElement = view
        .document
        .create_element("button")
        .unwrap()
        .unchecked_into();
    parent.append_child(&before).unwrap();
    let root = card.mount(view).unwrap();
    parent.append_child(&root.node).unwrap();
    parent.append_child(&after).unwrap();
    let controls = buttons(&root.node.clone().unchecked_into());
    before.focus().unwrap();
    driver("driverPress", "Tab").await;
    assert_eq!(
        view.document.active_element().unwrap(),
        controls[0].clone().into()
    );
    call(page, "finish", &js_sys::Array::of1(&controls[0])).await;
    let accent = rgb(view.pack.presentation(view.mode).unwrap().colors["accent"]);
    assert_ne!(computed(&controls[0], "box-shadow"), "none");
    assert_eq!(computed(&controls[0], "border-top-color"), accent);
    driver("driverPress", "Enter").await;
    assert_eq!(count.get(), 1);
    for expected in [&controls[1], &controls[2], &after] {
        driver("driverPress", "Tab").await;
        assert_eq!(
            view.document.active_element().unwrap(),
            expected.clone().into()
        );
    }
    driver("driverPress", "Shift+Tab").await;
    assert_eq!(
        view.document.active_element().unwrap(),
        controls[2].clone().into()
    );
    reverse_tabs(view, [&controls[1], &controls[0], &before]).await;
    hover_button(view, page, &controls[1]).await;
    parent.remove();
}
/// The shared native button paints its hover surface.
async fn hover_button(view: &DomView, page: &JsValue, button: &HtmlElement) {
    let rect = button.get_bounding_client_rect();
    driver(
        "driverPointer",
        &format!(
            "[{},{},false]",
            rect.x() + rect.width() / 2.0,
            rect.y() + rect.height() / 2.0
        ),
    )
    .await;
    call(page, "finish", &js_sys::Array::of1(button)).await;
    assert_eq!(
        computed(button, "background-color"),
        rgb(view.pack.presentation(view.mode).unwrap().colors["panel"])
    );
}

/// Mount one state and inspect its rendered nodes before releasing it.
fn paint_instance(
    parent: &Element,
    view: &DomView,
    presentation: &BrandPresentation<'_>,
    card: &CustomProviderCard,
) {
    let root = card.mount(view).unwrap();
    parent.append_child(&root.node).unwrap();
    let element: Element = root.node.unchecked_into();
    painted_parts(&element, presentation);
    if card.is_auto_discovery {
        painted_dot(&element, presentation, card.status);
    } else {
        let status = element
            .query_selector(".maestro-provider-status")
            .unwrap()
            .unwrap();
        assert_eq!(computed(&status, "display"), "block");
    }
    element.remove();
}

/// Reverse traversal leaves the card through its original preceding control.
async fn reverse_tabs(view: &DomView, expected: [&HtmlElement; 3]) {
    for control in expected {
        driver("driverPress", "Shift+Tab").await;
        assert_eq!(
            view.document.active_element().unwrap(),
            control.clone().into()
        );
    }
}
