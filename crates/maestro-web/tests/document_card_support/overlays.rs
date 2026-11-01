//! Independently opened native previews survive errors and containing-row removal.
#![cfg(all(test, target_arch = "wasm32"))]
use super::{
    browser::Card,
    downloads::{close, open},
    file_subset,
    forwarding::{Header, TileRecipe, registration},
};
use crate::contributions_support::{container, host, select, text, view};
use maestro_web::{
    AssistantMessage, ExpandableSection, ToolMessage, UserMessage,
    dom::{DomContent, DomView, MountedContent},
};
use std::{
    cell::Cell,
    rc::Rc,
    sync::{Arc, RwLock},
};
use wasm_bindgen::JsCast as _;
use web_sys::HtmlElement;

/// An assistant with one reached registered-tool call.
fn assistant() -> maestro_app::SharedAssistantMessage {
    Arc::new(RwLock::new(serde_json::from_value(serde_json::json!({"role":"assistant","content":[{"type":"toolCall","id":"call","name":"document-owner","arguments":{}}],"api":"openai-completions","provider":"openai","model":"controlled","usage":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}},"stopReason":"stop","timestamp":0})).unwrap()))
}
/// Open before failing a refresh, then release the containing row without closing its preview.
fn survives(content: &dyn DomContent, fail: &Cell<bool>, view: &DomView) {
    let parent = container();
    let mut slot = MountedContent::default();
    slot.show(content, &parent, view).unwrap();
    select(&parent, "[data-open]")
        .unchecked_ref::<HtmlElement>()
        .click();
    let overlay = view
        .document
        .query_selector("attachment-overlay")
        .unwrap()
        .unwrap();
    fail.set(true);
    assert!(slot.show(content, &parent, view).is_err());
    slot.clear();
    parent.remove();
    assert!(overlay.is_connected());
    assert!(text(&overlay).contains("<literal>"));
    close(&overlay);
}
/// Assistant, rich header, captured section and custom-role paths keep independent native effects.
pub fn assistant_and_children(import: &js_sys::Function) {
    let view = view();
    let _elements = host(&view, import);
    for header in [Header::Plain, Header::Rich] {
        let fail = registration("document-owner", header);
        let row = AssistantMessage::<serde_json::Value> {
            message: Some(assistant()),
            ..AssistantMessage::default()
        };
        survives(&row, &fail, &view);
    }
    let fail = Rc::new(Cell::new(false));
    let record = crate::attachment_support::attachment();
    let child = TileRecipe {
        record: record.clone(),
        fail: fail.clone(),
    };
    let mut section = ExpandableSection::new(vec![Box::new(child)]);
    section.default_expanded = true;
    survives(&section, &fail, &view);
    fail.set(false);
    custom(record, &fail, &view);
    drop(
        maestro_web::tools::renderer_registry::TOOL_RENDERERS
            .with(|all| all.borrow_mut().shift_remove("document-owner")),
    );
}
/// The role map prepares the same original tile, without a tool dispatch or JSON transport.
fn custom(record: maestro_web::SharedAttachment, fail: &Rc<Cell<bool>>, view: &DomView) {
    use crate::message_renderer_support::{Renderer, message};
    let captured = fail.clone();
    maestro_web::register_message_renderer(
        "document-custom",
        Rc::new(Renderer(Rc::new(move |_| {
            Ok(Box::new(TileRecipe {
                record: record.clone(),
                fail: captured.clone(),
            }))
        }))),
    );
    let input = message(
        "document-custom",
        Arc::new(std::sync::atomic::AtomicU32::new(0)),
    );
    let recipe = maestro_web::render_message(&input).unwrap().unwrap();
    survives(recipe.as_ref(), fail, view);
    maestro_web::register_message_renderer(
        "document-custom",
        Rc::new(Renderer(Rc::new(|_| {
            Ok(Box::new(crate::contributions_support::Text("retired")))
        }))),
    );
}
/// Image and text files open independently while their tool disclosure stays collapsed.
pub fn returned_files(import: &js_sys::Function) {
    let view = view();
    let _elements = host(&view, import);
    let card = Card::new(view, &file_subset(&[0, 1, 2]));
    assert!(
        select(&card.root(), ".maestro-tool-body")
            .class_list()
            .contains("max-h-0")
    );
    let overlays: Vec<_> = (0..3).map(|index| open(&card, index)).collect();
    drop(card);
    for overlay in &overlays {
        assert!(overlay.is_connected());
    }
    assert!(
        overlays[0]
            .query_selector("img[alt=\"image.png\"]")
            .unwrap()
            .is_some()
    );
    assert!(text(&overlays[1]).contains('雪'));
    for overlay in overlays.iter().rev() {
        close(overlay);
    }
}
/// User and tool rows use the same page-owned overlay route, not a containing canvas receiver.
pub fn user_and_tool(import: &js_sys::Function) {
    use maestro_app::{
        AgentMessage, UserContent,
        browser::components::messages::{CustomAgentMessages, UserMessageWithAttachments},
    };
    let view = view();
    let _elements = host(&view, import);
    let record = crate::attachment_support::attachment();
    record.write().unwrap().file_name = "user.txt".into();
    let user = UserMessage::<maestro_app::NoCustomMessages> {
        message: Some(AgentMessage::Custom(Arc::new(RwLock::new(
            CustomAgentMessages::UserWithAttachments(UserMessageWithAttachments {
                content: UserContent::Text(String::new()),
                timestamp: 0.,
                attachments: Some(vec![record]),
            }),
        )))),
    };
    let tool = ToolMessage {
        tool_call: Some(Rc::new(serde_json::from_value(serde_json::json!({"type":"toolCall","id":"call","name":"javascript_repl","arguments":{"title":"Download","code":""}})).unwrap())),
        result: Some(Rc::new(crate::recipes::message(Some(serde_json::json!({"files":[{"fileName":"tool.txt","mimeType":"text/plain","size":3,"contentBase64":"6Zuq"}]}))))),
        ..ToolMessage::new()
    };
    let mut overlays = Vec::new();
    for content in [&user as &dyn DomContent, &tool] {
        let parent = container();
        let mut slot = MountedContent::default();
        slot.show(content, &parent, &view).unwrap();
        select(&parent, "[data-open]")
            .unchecked_ref::<HtmlElement>()
            .click();
        overlays.push(
            view.document
                .query_selector_all("attachment-overlay")
                .unwrap()
                .item(u32::try_from(overlays.len()).unwrap())
                .unwrap()
                .unchecked_into::<web_sys::Element>(),
        );
        slot.clear();
        parent.remove();
    }
    drop((user, tool));
    assert!(text(&overlays[0]).contains("<literal>"));
    assert!(text(&overlays[1]).contains('雪'));
    for overlay in overlays.iter().rev() {
        assert!(overlay.is_connected());
        close(overlay);
    }
}
