//! Native document disclosures retain selected children, selection and independent controls.
#![cfg(all(test, target_arch = "wasm32"))]
use super::{Row, browser::Card, fixture};
use crate::contributions_support::{host, select, text, view};
use maestro_web::dom::DomContent as _;
use wasm_bindgen::JsCast as _;

/// One original fixture by its stable source identity.
fn row(id: &str) -> Row {
    fixture().into_iter().find(|r| r.id == id).unwrap()
}
/// Observe real native height transitions through the shared animation driver.
pub async fn transitions(import: &js_sys::Function) {
    use crate::contributions_support::{call, page_module};
    let view = view();
    let _elements = host(&view, import);
    let page = page_module(import).await;
    let mut row = row("repl-params-valid-true");
    row.input
        .params
        .as_mut()
        .unwrap()
        .insert("code".into(), serde_json::json!("line\n".repeat(100)));
    let card = Card::new(view, &row);
    let code = select(&card.root(), ".maestro-code");
    let body = select(&card.root(), ".maestro-tool-body");
    assert!(body.get_bounding_client_rect().height() < 0.5);
    card.toggle();
    let duration = call(
        &page,
        "seek",
        &js_sys::Array::of3(&body, &"max-height".into(), &75.into()),
    )
    .await;
    assert_eq!(duration.as_f64(), Some(300.));
    let partial = body.get_bounding_client_rect().height();
    assert!(partial > 0. && partial < f64::from(body.scroll_height()));
    card.toggle();
    call(
        &page,
        "seek",
        &js_sys::Array::of3(&body, &"max-height".into(), &0.into()),
    )
    .await;
    assert!((body.get_bounding_client_rect().height() - partial).abs() < 1.);
    call(
        &page,
        "seek",
        &js_sys::Array::of3(&body, &"max-height".into(), &300.into()),
    )
    .await;
    assert!(body.get_bounding_client_rect().height() < 0.5);
    card.toggle();
    call(
        &page,
        "seek",
        &js_sys::Array::of3(&body, &"max-height".into(), &300.into()),
    )
    .await;
    assert!(select(&card.root(), ".maestro-code").is_same_node(Some(&code)));
    assert!(body.get_bounding_client_rect().height() > 0.);
}
/// Pending/result/absent branches rebuild only their selected subtrees.
pub fn branches(import: &js_sys::Function) {
    let view = view();
    let _elements = host(&view, import);
    let mut row = row("repl-params-valid-false");
    let mut card = Card::new(view, &row);
    card.toggle();
    let pending = select(&card.root(), ".maestro-code");
    let caption = select(&card.root(), ".maestro-tool-caption");
    row.input
        .params
        .as_mut()
        .unwrap()
        .insert("code".into(), serde_json::json!("changed literal"));
    card.show(&row);
    assert!(select(&card.root(), ".maestro-code").is_same_node(Some(&pending)));
    assert_eq!(text(&select(&pending, "code")), "changed literal");
    row.input.result = Some(crate::recipes::message::<serde_json::Value>(None));
    card.show(&row);
    assert!(!pending.is_connected() && !caption.is_connected());
    let completed = select(&card.root(), ".maestro-code");
    let caption = select(&card.root(), ".maestro-tool-caption");
    card.toggle();
    row.input.result.as_mut().unwrap().is_error = false;
    card.show(&row);
    assert!(select(&card.root(), ".maestro-code").is_same_node(Some(&completed)));
    assert!(select(&card.root(), ".maestro-tool-caption").is_same_node(Some(&caption)));
    row.input.params = None;
    card.show(&row);
    assert!(
        card.root()
            .query_selector(".maestro-code")
            .unwrap()
            .is_none()
    );
    assert!(!completed.is_connected() && !caption.is_connected());
}
/// Replacing the renderer's content type releases the old header, code and expansion.
pub fn renderer_switch(import: &js_sys::Function) {
    let view = view();
    let _elements = host(&view, import);
    let mut card = Card::new(view, &row("repl-result-valid-false-false"));
    card.toggle();
    let header = select(&card.root(), ".maestro-tool-caption");
    let code = select(&card.root(), ".maestro-code");
    card.show(&row("extract-result-valid-false-false"));
    assert!(!header.is_connected() && !code.is_connected());
    assert!(
        select(&card.root(), ".maestro-tool-body")
            .class_list()
            .contains("max-h-0")
    );
}
/// Independent rows retain their own selection when another row is removed.
pub async fn independent(import: &js_sys::Function) {
    let view = view();
    let _elements = host(&view, import);
    let first = Card::new(view.clone(), &row("repl-params-valid-false"));
    let mut second = Card::new(view.clone(), &row("extract-params-valid-false"));
    let code = select(&first.root(), ".maestro-code");
    let url = select(&second.root(), ".maestro-tool-url");
    first.toggle();
    assert!(
        select(&second.root(), ".maestro-tool-body")
            .class_list()
            .contains("max-h-0")
    );
    second.toggle();
    let page = crate::contributions_support::page_module(import).await;
    let before = crate::contributions_support::call(&page, "select", &js_sys::Array::of1(&url))
        .await
        .as_string()
        .unwrap();
    assert!(!before.is_empty());
    drop(first);
    assert!(!code.is_connected() && url.is_connected());
    second.show(&row("extract-params-valid-false"));
    assert!(select(&second.root(), ".maestro-tool-url").is_same_node(Some(&url)));
    assert_eq!(
        crate::contributions_support::selection(&page, &url).await.0,
        before
    );
    drop(second);
    assert!(!url.is_connected());
}
/// Native keyboard and clipboard routes keep selection while the body toggles and a modal covers it.
pub async fn controls(import: &js_sys::Function) {
    use crate::contributions_support::{call, driver, page_module};
    let view = view();
    let _elements = host(&view, import);
    let page = page_module(import).await;
    let input = row("repl-result-valid-false-false");
    let card = Card::new(view.clone(), &input);
    let header = card
        .root()
        .first_element_child()
        .unwrap()
        .unchecked_into::<web_sys::HtmlElement>();
    let toggle = crate::contributions_support::disclosure(&header).unwrap();
    toggle.focus().unwrap();
    driver("driverPress", "Space").await;
    assert!(toggle.is_same_node(view.document.active_element().as_deref()));
    let body = select(&card.root(), ".maestro-tool-body");
    call(&page, "finish", &js_sys::Array::of1(&body)).await;
    let code = select(&card.root(), ".maestro-code");
    let before = call(&page, "select", &js_sys::Array::of1(&select(&code, "code")))
        .await
        .as_string()
        .unwrap();
    assert_eq!(before, input.expected.as_ref().unwrap().blocks[0].code);
    copies(&card, &input, &page).await;
    toggle.focus().unwrap();
    for _ in 0..2 {
        driver("driverPress", "Space").await;
    }
    call(&page, "finish", &js_sys::Array::of1(&body)).await;
    assert!(select(&card.root(), ".maestro-code").is_same_node(Some(&code)));
    assert_eq!(
        crate::contributions_support::selection(&page, &code)
            .await
            .0,
        before
    );
    modal(&card, &header).await;
}

/// Both native copy controls write the original operands and show their own feedback.
async fn copies(card: &Card, input: &Row, page: &wasm_bindgen::JsValue) {
    use crate::contributions_support::{Clipboard, Recorded, until};
    let clipboard = Recorded::install(Clipboard::Accept);
    for (selector, expected) in [
        (
            ".maestro-code",
            &input.expected.as_ref().unwrap().blocks[0].code,
        ),
        (
            ".maestro-console",
            &input.expected.as_ref().unwrap().console[0].content,
        ),
    ] {
        let copy = select(&select(&card.root(), selector), "copy-button");
        select(&copy, "button")
            .unchecked_ref::<web_sys::HtmlElement>()
            .click();
        until(page, &copy, || text(&copy).contains("Copied!")).await;
        assert_eq!(clipboard.texts().last().unwrap(), expected);
    }
}

/// A native overlay intercepts pointer activation without disconnecting the underlying code.
async fn modal(card: &Card, header: &web_sys::HtmlElement) {
    use crate::contributions_support::driver;
    let code = select(&card.root(), ".maestro-code");
    let overlay = maestro_web::AttachmentOverlay::open(
        &card.view,
        crate::attachment_support::attachment(),
        None,
    )
    .unwrap();
    let root = overlay.mount(&card.view).unwrap();
    let expanded = select(&card.root(), ".maestro-tool-body").class_name();
    let rect = header.get_bounding_client_rect();
    driver(
        "driverPointer",
        &format!("[{}, {}, true]", rect.x() + 10., rect.y() + 10.),
    )
    .await;
    assert_eq!(
        select(&card.root(), ".maestro-tool-body").class_name(),
        expanded
    );
    super::downloads::close(root.node.unchecked_ref());
    assert!(code.is_connected());
    crate::contributions_support::disclosure(header)
        .unwrap()
        .focus()
        .unwrap();
    driver("driverPress", "Space").await;
    assert_ne!(
        select(&card.root(), ".maestro-tool-body").class_name(),
        expanded
    );
}
