//! Attachment thumbnails exercised through their native page controls.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::attachment_support;
use attachment_support::Input;
use serde::Deserialize;
/// Expected tile output.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Expected {
    /// Complete preview URL.
    preview: Option<String>,
    /// Catalog glyph.
    icon: Option<String>,
    /// Complete clipped label.
    label: Option<String>,
    /// PDF-only badge.
    pdf_badge: bool,
    /// Remove control presence.
    remove: bool,
}
/// One source-derived tile case.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Case {
    /// Case identity used for assertion context.
    name: String,
    /// Original attachment.
    input: Input,
    /// Whether deletion is enabled.
    show_delete: bool,
    /// Complete expected projection.
    expected: Expected,
}
/// Select distinct observations without duplicating each fixture's owner.
fn cases() -> Vec<Case> {
    serde_json::from_str(include_str!("fixtures/attachment_tiles.json")).unwrap()
}
/// Replay the tile corpus through native controls rather than drawing placements.
#[cfg(target_arch = "wasm32")]
pub(crate) fn native_cases(view: &maestro_web::dom::DomView) {
    use maestro_web::dom::DomContent as _;
    use wasm_bindgen::JsCast as _;
    for case in cases() {
        let attachment = case.input.record();
        let name = attachment.read().unwrap().file_name.clone();
        let mut tile = maestro_web::components::attachment_tile::AttachmentTile::from(attachment);
        tile.show_delete = case.show_delete;
        let root = tile.mount(view).unwrap();
        let node: web_sys::Element = root.node.clone().unchecked_into();
        assert_eq!(
            node.query_selector("[data-remove]").unwrap().is_some(),
            case.expected.remove,
            "{}",
            case.name
        );
        let target = node.query_selector("[data-open]").unwrap().unwrap();
        assert_eq!(
            target.get_attribute("title").as_deref(),
            Some(name.as_str()),
            "{}",
            case.name
        );
        if let Some(expected) = case.expected.preview {
            let image = node.query_selector("img").unwrap().unwrap();
            assert_eq!(
                image.get_attribute("src").unwrap(),
                expected,
                "{}",
                case.name
            );
            assert_eq!(image.get_attribute("alt").unwrap(), name, "{}", case.name);
        } else {
            assert_eq!(
                node.query_selector(".maestro-tile-label")
                    .unwrap()
                    .unwrap()
                    .text_content()
                    .unwrap(),
                case.expected.label.unwrap(),
                "{}",
                case.name
            );
            native_glyph(&node, case.expected.icon.as_ref().unwrap());
        }
        assert_eq!(
            node.query_selector(".maestro-tile-badge")
                .unwrap()
                .is_some(),
            case.expected.pdf_badge,
            "{}",
            case.name
        );
    }
}

/// Native geometry follows the same catalog glyph as the retained corpus.
#[cfg(target_arch = "wasm32")]
fn native_glyph(node: &web_sys::Element, icon: &str) {
    let expected = maestro_web::icons::icons()
        .unwrap()
        .swap_remove(icon)
        .unwrap();
    let expected =
        maestro_web::icons::icon_dom(&expected, maestro_web::icons::IconSize::Md, None).unwrap();
    assert_eq!(
        node.query_selector("svg").unwrap().unwrap().inner_html(),
        expected.inner_html()
    );
}

/// Attachments remain indivisible fixed-size items when the native row wraps.
pub(crate) fn user_attachment_tiles_wrap_as_single_fixed_items(view: &maestro_web::dom::DomView) {
    use crate::contributions_support::{body, select, text};
    use maestro_web::dom::DomContent as _;
    use wasm_bindgen::JsCast as _;
    let records: Vec<_> = (0..14)
        .map(|index| {
            let record = attachment_support::attachment();
            record.write().unwrap().file_name = format!("wrapped-{index}.png");
            record.write().unwrap().preview = Some("AA==".into());
            record
        })
        .collect();
    let row = wrapped_row(&records);
    let root = row.mount(view).unwrap();
    let parent = view.document.create_element("div").unwrap();
    parent.set_attribute("style", "width:320px").unwrap();
    body().append_child(&parent).unwrap();
    parent.append_child(&root.node).unwrap();
    let tiles = root
        .node
        .unchecked_ref::<web_sys::Element>()
        .query_selector_all("[data-open]")
        .unwrap();
    assert_eq!(tiles.length(), 14);
    let top = tiles
        .item(0)
        .unwrap()
        .unchecked_into::<web_sys::Element>()
        .get_bounding_client_rect();
    for index in 0..14 {
        let rect = tiles
            .item(index)
            .unwrap()
            .unchecked_into::<web_sys::Element>()
            .get_bounding_client_rect();
        assert_eq!((rect.width(), rect.height()), (64., 64.));
        let bounds = parent.get_bounding_client_rect();
        assert!(rect.left() >= bounds.left() && rect.right() <= bounds.right());
    }
    let last: web_sys::HtmlElement = tiles.item(13).unwrap().unchecked_into();
    assert!(last.get_bounding_client_rect().top() > top.bottom());
    records[13].write().unwrap().file_name = "current last attachment".into();
    last.click();
    let overlay = view
        .document
        .query_selector("attachment-overlay")
        .unwrap()
        .unwrap();
    assert_eq!(
        text(&select(&overlay, ".maestro-attachment-name")),
        "current last attachment"
    );
    parent.remove();
    drop((row, root, records));
    assert!(overlay.is_connected());
    select(&overlay, "[data-close]")
        .unchecked_ref::<web_sys::HtmlElement>()
        .click();
    assert!(!overlay.is_connected());
}

/// The badge intercepts pointer clicks while the thumbnail keeps its own hover and opening route.
pub(crate) async fn tile_badge_keeps_image_pointer_route(
    view: &maestro_web::dom::DomView,
    import_module: &js_sys::Function,
) {
    use crate::contributions_support::{body, call, computed, driver, page_module, select};
    use maestro_web::{AttachmentTile, dom::DomContent as _};
    use wasm_bindgen::JsCast as _;
    let record = attachment_support::attachment();
    record.write().unwrap().preview = Some("AA==".into());
    record.write().unwrap().mime_type = "application/pdf".into();
    let root = AttachmentTile::from(record).mount(view).unwrap();
    body().append_child(&root.node).unwrap();
    let page = page_module(import_module).await;
    let target = select(&root.node, "[data-open]");
    let badge = select(&root.node, ".maestro-tile-badge");
    let rect = badge.get_bounding_client_rect();
    driver(
        "driverPointer",
        &format!(
            "[{}, {}, true]",
            rect.x() + rect.width() / 2.,
            rect.y() + rect.height() / 2.
        ),
    )
    .await;
    call(&page, "finish", &js_sys::Array::of1(&target)).await;
    assert_eq!(computed(&target, "opacity"), "1");
    assert!(
        view.document
            .query_selector("attachment-overlay")
            .unwrap()
            .is_none()
    );
    let rect = target.get_bounding_client_rect();
    driver(
        "driverPointer",
        &format!("[{}, {}, false]", rect.x() + 10., rect.y() + 10.),
    )
    .await;
    call(&page, "finish", &js_sys::Array::of1(&target)).await;
    assert_eq!(computed(&target, "opacity"), "0.8");
    driver(
        "driverPointer",
        &format!("[{}, {}, true]", rect.x() + 10., rect.y() + 10.),
    )
    .await;
    let overlay = view
        .document
        .query_selector("attachment-overlay")
        .unwrap()
        .unwrap();
    select(&overlay, "[data-close]")
        .unchecked_ref::<web_sys::HtmlElement>()
        .click();
    assert!(!overlay.is_connected());
    root.node.unchecked_ref::<web_sys::Element>().remove();
}

/// Construct an attachment-bearing row with the original handles, not serialized copies.
fn wrapped_row(records: &[maestro_web::SharedAttachment]) -> maestro_web::UserMessage {
    use maestro_app::{
        AgentMessage, UserContent,
        browser::components::messages::{CustomAgentMessages, UserMessageWithAttachments},
    };
    use maestro_web::UserMessage;
    UserMessage::<maestro_app::NoCustomMessages> {
        message: Some(AgentMessage::Custom(std::sync::Arc::new(
            std::sync::RwLock::new(CustomAgentMessages::UserWithAttachments(
                UserMessageWithAttachments {
                    content: UserContent::Text(String::new()),
                    timestamp: 0.,
                    attachments: Some(records.to_vec()),
                },
            )),
        ))),
    }
}
