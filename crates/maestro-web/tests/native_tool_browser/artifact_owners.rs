//! Artifact cards own their prepared inputs and keep their console across body changes.
use super::optional::Panel;
use crate::contributions_support::{body, select, text};
use maestro_app::{JsonObject, browser::tools::artifacts::ArtifactsParams};
use maestro_web::{
    ArtifactsToolRenderer,
    dom::{DomView, MountedContent},
    tools::types::{ToolRenderResult, ToolRenderer},
};
use serde_json::{Value, json};
use std::rc::Rc;
use wasm_bindgen::JsCast as _;
use web_sys::{Element, HtmlElement};

/// Prepare all three argument families before their source values change and drop.
fn prepared(renderer: &ArtifactsToolRenderer, filename: &str) -> [ToolRenderResult; 3] {
    let mut params = json!({"command":"create", "filename":filename, "content":"YWJj <&> 雪"});
    let object: JsonObject = serde_json::from_value(params.clone()).unwrap();
    let typed: ArtifactsParams = serde_json::from_value(params.clone()).unwrap();
    let prepared = [
        <ArtifactsToolRenderer as ToolRenderer<Value, ()>>::render(
            renderer,
            Some(&params),
            None,
            false,
        )
        .unwrap(),
        <ArtifactsToolRenderer as ToolRenderer<JsonObject, ()>>::render(
            renderer,
            Some(&object),
            None,
            false,
        )
        .unwrap(),
        <ArtifactsToolRenderer as ToolRenderer<ArtifactsParams, ()>>::render(
            renderer,
            Some(&typed),
            None,
            false,
        )
        .unwrap(),
    ];
    params["filename"] = "changed.rs".into();
    params["content"] = "changed".into();
    drop((params, object, typed));
    prepared
}

/// Each argument family keeps the filename and literal content it was prepared with and opens
/// the panel captured at preparation, not a later replacement; a released pill opens nothing.
pub(super) fn adapters_own_inputs(view: &DomView) {
    let panel = Panel::new();
    let filename = "literal <&> 雪.rs";
    panel.create(filename);
    let mut renderer = ArtifactsToolRenderer::new(Some(Rc::clone(&panel.panel)));
    let cards = prepared(&renderer, filename);
    let replacement = Panel::new();
    renderer.artifacts_panel = Some(Rc::clone(&replacement.panel));
    drop(renderer);
    for (index, card) in cards.iter().enumerate() {
        panel.panel.show_artifact("sentinel");
        let root = card.content.mount(view).unwrap();
        body().append_child(&root.node).unwrap();
        let element: &Element = root.node.unchecked_ref();
        let pill: HtmlElement = select(element, ".maestro-artifact-pill").unchecked_into();
        assert_eq!(text(&pill), filename);
        assert_eq!(
            text(&select(element, ".maestro-code pre code")),
            "YWJj <&> 雪"
        );
        pill.click();
        assert_eq!(panel.panel.active_filename().as_deref(), Some(filename));
        assert_eq!(panel.presentation.opened.get(), index + 1);
        assert_eq!(replacement.presentation.opened.get(), 0);
        element.remove();
        drop(root);
        pill.click();
        assert_eq!(
            panel.presentation.opened.get(),
            index + 1,
            "a released pill opened"
        );
    }
}

/// An artifact card without a filename shows its literal caption and no pill.
pub(super) fn caption_without_pill(view: &DomView) {
    let params = json!({"command":"create","content":"literal code"});
    let renderer = ArtifactsToolRenderer::new(None);
    let parent = crate::contributions_support::container();
    let mut slot = MountedContent::default();
    let mut caption = None::<Element>;
    for streaming in [true, false, true] {
        let card = <ArtifactsToolRenderer as ToolRenderer<Value, ()>>::render(
            &renderer,
            Some(&params),
            None,
            streaming,
        )
        .unwrap();
        slot.show(card.content.as_ref(), &parent, view).unwrap();
        let root = slot.root().unwrap();
        let current = select(root, ".maestro-tool-caption");
        assert_eq!(text(&current), "Creating artifact");
        assert!(
            root.unchecked_ref::<Element>()
                .query_selector(".maestro-artifact-pill")
                .unwrap()
                .is_none()
        );
        if let Some(caption) = &caption {
            assert!(
                caption.is_same_node(Some(&current)),
                "status-only updates keep the caption"
            );
        }
        caption = Some(current);
    }
    slot.clear();
    parent.remove();
}

/// An HTML artifact's console keeps its element and scroll position through same-content
/// updates, an emptied listing and collapsing and reopening the card.
pub(super) fn console_scroll(view: &DomView) {
    let mut result = crate::artifact_card_support::phase("success").unwrap();
    result
        .content
        .push(maestro_app::UserBlock::Text(maestro_app::TextContent::new(
            "log\n".repeat(100),
        )));
    let card = |content: &str| {
        let params = json!({"command":"create","filename":"a.html","content":content});
        <ArtifactsToolRenderer as ToolRenderer<Value, Value>>::render(
            &ArtifactsToolRenderer::new(None),
            Some(&params),
            Some((&result).into()),
            false,
        )
        .unwrap()
    };
    let parent = crate::contributions_support::container();
    let mut slot = MountedContent::default();
    let first = card("listing");
    slot.show(first.content.as_ref(), &parent, view).unwrap();
    let root: Element = slot.root().unwrap().clone().unchecked_into();
    super::header_of(&root).click();
    let console = select(&root, ".maestro-console");
    let code = select(&root, ".maestro-code");
    let scroll: HtmlElement = select(&console, ".console-scroll").unchecked_into();
    scroll.set_scroll_top(20);
    let kept = || {
        assert!(select(&root, ".maestro-console").is_same_node(Some(&console)));
        assert_eq!(scroll.scroll_top(), 20);
    };
    slot.show(card("listing").content.as_ref(), &parent, view)
        .unwrap();
    kept();
    slot.show(card("").content.as_ref(), &parent, view).unwrap();
    assert!(!code.is_connected(), "the emptied listing is released");
    kept();
    for _ in 0..2 {
        super::header_of(&root).click();
        kept();
    }
    slot.clear();
    assert!(!console.is_connected());
    parent.remove();
}
