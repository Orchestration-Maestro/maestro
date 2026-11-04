//! Native rendering and keyboard observations of the complete visible-state inventory.
use super::{buttons, provider};
use crate::contributions_support::{
    computed, container, driver, host, rgb, select, shadow_paints, text, view,
};
use maestro_theme::{BrandMode, BrandPresentation};
use maestro_web::{
    CustomProviderCard,
    components::custom_provider_card::ProviderStatus,
    dom::{DomContent, DomView},
};
use std::{cell::Cell, rc::Rc};
use wasm_bindgen::JsCast as _;
use web_sys::{Element, HtmlElement};

/// Exercise both packs, both modes and the independent native branches.
pub(super) async fn check(import: js_sys::Function) {
    let view = view();
    let _host = host(&view, &import);
    for pack in ["brand.json", "signal.json"] {
        let pack = crate::brand::pack(pack).unwrap();
        for mode in [BrandMode::Dark, BrandMode::Light] {
            paint_sizes(&view, &pack.presentation(mode).unwrap());
        }
    }
    keyboard_card(&view).await;
    offscreen_text(&view);
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
    painted_card(root, presentation);
    painted_actions(root, presentation);
}
/// The card paints its own border around measured text, without a clipping surface.
///
/// The page snaps border widths to whole device pixels under its scale and zoom, so the shipped
/// width is the one a probe beside the card resolves.
fn painted_card(root: &Element, presentation: &BrandPresentation<'_>) {
    let name = select(root, ".maestro-provider-name");
    let details = select(root, ".maestro-provider-details");
    let border = rgb(presentation.colors["border"]);
    assert_eq!(
        computed(root, "border-top-style"),
        "solid",
        "the card paints its own border"
    );
    let parent = root.parent_element().unwrap();
    assert_eq!(
        computed(root, "border-top-width"),
        resolved(&parent, ("border-top", "1px solid"), "border-top-width"),
        "the card paints its own border"
    );
    assert_eq!(
        computed(root, "border-top-color"),
        border,
        "the card paints its own border"
    );
    assert_eq!(computed(root, "background-color"), "rgba(0, 0, 0, 0)");
    for overflow in ["overflow-x", "overflow-y"] {
        assert_eq!(
            computed(root, overflow),
            "visible",
            "the card adds no clipping surface"
        );
    }
    assert_eq!(computed(&name, "color"), rgb(presentation.colors["text"]));
    assert_eq!(
        computed(&details, "color"),
        rgb(presentation.colors["muted"])
    );
    let bounds = root.get_bounding_client_rect();
    for painted in [&name, &details] {
        let rect = painted.get_bounding_client_rect();
        assert!(
            rect.width() > 0.0 && rect.height() > 0.0,
            "text paints its box"
        );
        assert!(
            rect.top() >= bounds.top() && rect.bottom() <= bounds.bottom(),
            "text paints inside the card"
        );
    }
    assert!(details.get_bounding_client_rect().top() >= name.get_bounding_client_rect().bottom());
}
/// Every action paints its own label inside the shared control without clipping it.
fn painted_actions(root: &Element, presentation: &BrandPresentation<'_>) {
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
    // Zoomed bounding rectangles carry native floating-point rounding.
    assert!((gap - 4.0 * scale).abs() < 0.03);
}
/// Offscreen caller text stays native, findable and selectable.
fn offscreen_text(view: &DomView) {
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
            Some(crate::contributions_support::page::find(literal)),
            Some(true)
        );
    }
    assert_eq!(
        crate::contributions_support::page::select_children(&root.node),
        "<b>雪</b>\nOllama • https://offscreen.test\nModels: 0"
    );
    parent.remove();
}
/// Tab and Shift-Tab traverse native actions once; Enter invokes exactly one action.
async fn keyboard_card(view: &Rc<DomView>) {
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
    crate::contributions_support::page::finish(&controls[0]);
    focus_ring(view, &parent, &controls[0]);
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
    hover_button(view, &controls[1]).await;
    parent.remove();
}
/// Focus turns the control border accent and paints the shipped accent blend around it.
fn focus_ring(view: &DomView, parent: &Element, control: &HtmlElement) {
    let accent = rgb(view.pack.presentation(view.mode).unwrap().colors["accent"]);
    assert!(shadow_paints(&computed(control, "box-shadow")));
    assert_eq!(
        computed(control, "border-top-color"),
        accent,
        "focus turns the control border accent"
    );
    let ring = resolved(
        parent,
        (
            "background-color",
            "color-mix(in srgb, var(--maestro-color-accent) 50%, transparent)",
        ),
        "background-color",
    );
    assert_ne!(ring, "rgba(0, 0, 0, 0)", "the page resolves the blend");
    assert!(
        computed(control, "box-shadow").contains(&ring),
        "focus paints the shipped accent blend around the control"
    );
}
/// The `property` value the page resolves one declaration to, read from a probe element
/// connected in `parent`.
fn resolved(parent: &Element, (name, declaration): (&str, &str), property: &str) -> String {
    let probe = parent
        .owner_document()
        .unwrap()
        .create_element("div")
        .unwrap();
    probe
        .unchecked_ref::<HtmlElement>()
        .style()
        .set_property(name, declaration)
        .unwrap();
    parent.append_child(&probe).unwrap();
    let value = computed(&probe, property);
    probe.remove();
    value
}
/// The shared native button paints its hover surface.
async fn hover_button(view: &DomView, button: &HtmlElement) {
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
    crate::contributions_support::page::finish(button);
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
