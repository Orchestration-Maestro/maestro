//! Native thumbnail geometry, supplied colors and listener ownership.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::{
    attachment_support::attachment,
    contributions_support::{body, computed, container, rgb, select, text},
};
use maestro_web::{
    AttachmentTile,
    dom::{DomContent as _, DomView},
};
use std::{cell::Cell, rc::Rc, sync::Arc};
use wasm_bindgen::JsCast as _;
use web_sys::{Element, HtmlElement};

/// Native fractional placement keeps the original image and clips it with cover sizing.
pub(crate) fn attachment_surfaces_keep_fractional_rects_and_owned_content(view: &DomView) {
    let record = attachment();
    record.write().unwrap().preview = Some("owned bytes".into());
    let tile = AttachmentTile::from(record);
    let root = tile.mount(view).unwrap();
    let parent = view.document.create_element("div").unwrap();
    parent
        .set_attribute("style", "position:absolute;left:5.25px;top:12.5px")
        .unwrap();
    body().append_child(&parent).unwrap();
    parent.append_child(&root.node).unwrap();
    let target = select(&root.node, "[data-open]");
    let rect = target.get_bounding_client_rect();
    assert_eq!(
        (rect.x(), rect.y(), rect.width(), rect.height()),
        (5.25, 12.5, 64., 64.)
    );
    let image = select(&root.node, "img");
    assert_eq!(
        image.get_attribute("src").unwrap(),
        "data:image/png;base64,owned bytes"
    );
    assert_eq!(computed(&image, "object-fit"), "cover");
    assert_eq!(computed(&target, "overflow"), "hidden");
    let image_bounds = image.get_bounding_client_rect();
    assert!(image_bounds.left() > rect.left() && image_bounds.right() < rect.right());
    assert!((60. ..64.).contains(&image_bounds.width()));
    drop(tile);
    assert!(image.is_connected());
    parent.remove();
}

/// Updating one placement is transactional, keeps its controls for the current inputs and does
/// not replace another placement's inputs.
pub(crate) fn attachment_tiles_replace_transactionally_and_release_listeners(view: &DomView) {
    let record = attachment();
    record.write().unwrap().preview = Some("first".into());
    let mut tile = AttachmentTile::from(record.clone());
    tile.show_delete = true;
    let calls = Rc::new(Cell::new(0));
    let count = calls.clone();
    let retained = record.clone();
    let weak = Arc::downgrade(&record);
    tile.on_delete = Some(Rc::new(move || {
        retained.write().unwrap().file_name = "callback edit".into();
        count.set(count.get() + 1);
    }));
    let first = tile.mount(view).unwrap();
    let second = tile.mount(view).unwrap();
    body().append_child(&first.node).unwrap();
    body().append_child(&second.node).unwrap();
    let old_remove = select(&first.node, "[data-remove]");
    let old_open = select(&first.node, "[data-open]");
    let guard = record.write().unwrap();
    assert_borrow_keeps_published_tile(&tile, &first, view);
    drop(guard);
    tile.attachment = attachment();
    tile.attachment.write().unwrap().preview = Some("second".into());
    tile.on_delete = None;
    tile.update(&first, view).unwrap();
    assert_eq!(
        select(&first.node, "img").get_attribute("src").unwrap(),
        "data:image/png;base64,second"
    );
    assert_eq!(
        select(&second.node, "img").get_attribute("src").unwrap(),
        "data:image/png;base64,first"
    );
    replace_preview_with_fallback(&mut tile, &first, view);
    kept_controls_use_current_inputs(&tile, &first, [&old_remove, &old_open], &calls, view);
    let live_remove = select(&second.node, "[data-remove]");
    live_remove.unchecked_ref::<HtmlElement>().click();
    assert_eq!(calls.get(), 1);
    assert_eq!(record.read().unwrap().file_name, "callback edit");
    first.node.unchecked_ref::<Element>().remove();
    drop(first);
    assert!(second.node.is_connected());
    drop((record, tile));
    assert!(weak.upgrade().is_some());
    second.node.unchecked_ref::<Element>().remove();
    drop(second);
    assert!(weak.upgrade().is_none());
    live_remove.unchecked_ref::<HtmlElement>().click();
    assert_eq!(calls.get(), 1);
}

/// Controls kept through an update act on the current inputs: removal reaches the current
/// callback, here none, and opening reads the current record.
fn kept_controls_use_current_inputs(
    tile: &AttachmentTile,
    root: &maestro_web::dom::DomRoot,
    [remove, open]: [&Element; 2],
    calls: &Cell<i32>,
    view: &DomView,
) {
    assert!(remove.is_same_node(Some(&select(&root.node, "[data-remove]"))));
    assert!(open.is_same_node(Some(&select(&root.node, "[data-open]"))));
    remove.unchecked_ref::<HtmlElement>().click();
    assert_eq!(calls.get(), 0, "the kept control uses the current callback");
    tile.attachment.write().unwrap().file_name = "current.txt".into();
    open.unchecked_ref::<HtmlElement>().click();
    let overlay = view
        .document
        .query_selector("attachment-overlay")
        .unwrap()
        .unwrap();
    assert_eq!(
        text(&select(&overlay, ".maestro-attachment-name")),
        "current.txt"
    );
    select(&overlay, "[data-close]")
        .unchecked_ref::<HtmlElement>()
        .click();
}

/// Each caller brand scope paints the native fallback, image frame, badge and removal boundary.
pub(crate) fn attachment_styles_follow_caller_brand_pack(view: &DomView) {
    for path in ["brand.json", "signal.json"] {
        for mode in [
            maestro_theme::BrandMode::Dark,
            maestro_theme::BrandMode::Light,
        ] {
            let pack = Rc::new(crate::brand::pack(path).unwrap());
            let current = DomView {
                pack,
                mode,
                ..view.clone()
            };
            let presentation = current.pack.presentation(mode).unwrap();
            let parent = container();
            for (key, value) in presentation.css_properties() {
                parent
                    .unchecked_ref::<HtmlElement>()
                    .style()
                    .set_property(&key, &value)
                    .unwrap();
            }
            let record = attachment();
            record.write().unwrap().file_name = "WWWWWWWWWW".into();
            let mut tile = AttachmentTile::from(record.clone());
            tile.show_delete = true;
            let root = tile.mount(&current).unwrap();
            parent.append_child(&root.node).unwrap();
            fallback_paint(&root.node, &presentation);
            record.write().unwrap().preview = Some("unchanged-pixels".into());
            record.write().unwrap().mime_type = "application/pdf".into();
            tile.update(&root, &current).unwrap();
            assert_eq!(
                select(&root.node, "img").get_attribute("src").unwrap(),
                "data:image/png;base64,unchanged-pixels"
            );
            let badge = select(&root.node, ".maestro-tile-badge");
            assert_eq!(text(&badge), "PDF");
            assert_eq!(computed(&badge, "color"), rgb(presentation.colors["muted"]));
            assert_eq!(
                computed(&badge, "border-bottom-left-radius"),
                format!("{}px", presentation.radii["panel"])
            );
            badge_paint(&badge, &presentation);
            parent.remove();
            selected_mode_paint(&current, &presentation);
        }
    }
}

/// An opened backdrop intercepts the tile without replacing its image; closing restores hit testing.
pub(crate) fn attachment_modal_hides_and_restores_existing_surfaces(view: &Rc<DomView>) {
    let record = attachment();
    record.write().unwrap().preview = Some("animated-original".into());
    let tile = AttachmentTile::from(record.clone());
    let root = tile.mount(view).unwrap();
    body().append_child(&root.node).unwrap();
    let image = select(&root.node, "img");
    let rect = image.get_bounding_client_rect();
    let hit = || {
        view.document
            .element_from_point(
                num_traits::cast(rect.x() + rect.width() / 2.).unwrap(),
                num_traits::cast(rect.y() + rect.height() / 2.).unwrap(),
            )
            .unwrap()
    };
    assert!(image.is_same_node(Some(&hit())));
    let overlay = maestro_web::AttachmentOverlay::open(view, record, None).unwrap();
    let opened = overlay.mount(view).unwrap();
    assert!(opened.node.contains(Some(&hit())));
    assert!(image.is_same_node(Some(&select(&root.node, "img"))));
    select(&opened.node, "[data-close]")
        .unchecked_ref::<HtmlElement>()
        .click();
    assert!(!opened.node.is_connected());
    assert!(image.is_same_node(Some(&hit())));
    assert_eq!(
        image.get_attribute("src").unwrap(),
        "data:image/png;base64,animated-original"
    );
    root.node.unchecked_ref::<Element>().remove();
}

/// Failed preparation leaves the containing root and its prior controls intact.
fn assert_borrow_keeps_published_tile(
    tile: &AttachmentTile,
    root: &maestro_web::dom::DomRoot,
    view: &DomView,
) {
    let target = select(&root.node, "[data-open]");
    assert!(matches!(
        tile.update(root, view),
        Err(maestro_web::tools::types::ToolUiError::Attachment(
            maestro_web::dialogs::attachment_overlay::AttachmentUiError::BorrowConflict
        ))
    ));
    assert!(target.is_same_node(Some(&select(&root.node, "[data-open]"))));
}

/// Native fallback text, clipping and removal keep the caller's visible brand boundary.
fn fallback_paint(root: &web_sys::Node, presentation: &maestro_theme::BrandPresentation<'_>) {
    let target = select(root, "[data-open]");
    assert_eq!(
        computed(&target, "background-color"),
        rgb(presentation.colors["panel"])
    );
    assert_eq!(
        computed(&target, "color"),
        rgb(presentation.colors["muted"])
    );
    assert_eq!(
        computed(&target, "border-top-color"),
        rgb(presentation.colors["border"])
    );
    assert_ne!(computed(&target, "border-top-width"), "0px");
    assert_eq!(
        computed(&target, "border-top-left-radius"),
        format!("{}px", presentation.radii["panel"])
    );
    let label = select(root, ".maestro-tile-label");
    assert_eq!(text(&label), "WWWWWWWWWW");
    assert_eq!(computed(&label, "text-overflow"), "ellipsis");
    assert_eq!(computed(&label, "white-space"), "nowrap");
    assert!(
        label.unchecked_ref::<HtmlElement>().scroll_width()
            > label.unchecked_ref::<HtmlElement>().client_width()
    );
    assert!(label.get_bounding_client_rect().width() <= 48.);
    let remove = select(root, "[data-remove]");
    assert_eq!(
        computed(&remove, "border-top-color"),
        rgb(presentation.colors["border"])
    );
    assert_ne!(computed(&remove, "border-top-width"), "0px");
    assert_eq!(computed(&remove, "border-top-left-radius"), "50%");
    assert_eq!(
        remove.get_bounding_client_rect().width().to_bits(),
        20_f64.to_bits()
    );
}

/// A preview-to-fallback update releases the old image without changing the placement root.
fn replace_preview_with_fallback(
    tile: &mut AttachmentTile,
    root: &maestro_web::dom::DomRoot,
    view: &DomView,
) {
    let image = select(&root.node, "img");
    tile.attachment.write().unwrap().preview = None;
    tile.update(root, view).unwrap();
    assert!(!image.is_connected());
    assert!(
        root.node
            .unchecked_ref::<Element>()
            .query_selector("img")
            .unwrap()
            .is_none()
    );
    assert_eq!(text(&select(&root.node, ".maestro-tile-label")), "note.txt");
}

/// The shared native mode owner keeps its quiet selected surface and readable foreground.
fn selected_mode_paint(view: &DomView, presentation: &maestro_theme::BrandPresentation<'_>) {
    let record = attachment();
    record.write().unwrap().mime_type = "application/pdf".into();
    let overlay =
        maestro_web::AttachmentOverlay::open(&Rc::new(view.clone()), record, None).unwrap();
    let root = overlay.mount(view).unwrap();
    let style = root.node.unchecked_ref::<HtmlElement>().style();
    for (key, value) in presentation.css_properties() {
        style.set_property(&key, &value).unwrap();
    }
    let selected = select(&root.node, ".maestro-toggle-selected");
    assert_eq!(text(&selected), "PDF");
    assert_eq!(
        computed(&selected, "background-color"),
        rgb(presentation.colors["background"])
    );
    assert_eq!(
        computed(&selected, "color"),
        rgb(presentation.colors["text"])
    );
    select(&root.node, "[data-close]")
        .unchecked_ref::<HtmlElement>()
        .click();
}

/// The badge keeps the translucent background role rather than the fallback tile's panel role.
fn badge_paint(badge: &Element, presentation: &maestro_theme::BrandPresentation<'_>) {
    let probe = badge
        .owner_document()
        .unwrap()
        .create_element("div")
        .unwrap();
    probe
        .unchecked_ref::<HtmlElement>()
        .style()
        .set_property(
            "background",
            &format!(
                "color-mix(in srgb, {} 90%, transparent)",
                presentation.colors["background"]
            ),
        )
        .unwrap();
    badge.append_child(&probe).unwrap();
    assert_eq!(
        computed(badge, "background-color"),
        computed(&probe, "background-color")
    );
    probe.remove();
    let target = badge
        .parent_element()
        .unwrap()
        .query_selector("[data-open]")
        .unwrap()
        .unwrap();
    assert!(
        (badge.get_bounding_client_rect().bottom() - target.get_bounding_client_rect().bottom())
            .abs()
            < 0.01
    );
    assert_eq!(
        badge.get_bounding_client_rect().width().to_bits(),
        64_f64.to_bits()
    );
}
