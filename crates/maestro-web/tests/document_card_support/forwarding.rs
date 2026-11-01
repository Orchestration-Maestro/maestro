//! Opened tile effects survive later errors in registered and rich native content.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::recipes;
use maestro_web::{
    AttachmentTile, ToolMessage, ToolRenderer,
    dom::{DomContent, DomRoot, DomView, MountedContent},
    tools::{
        renderer_registry::{ToolHeaderState, ToolHeaderText, render_header},
        types::{ToolRenderResult, ToolRendererBinding, ToolUiError},
    },
};
use std::{cell::Cell, rc::Rc};
use wasm_bindgen::JsCast as _;

/// Native tile whose next refresh can fail after updating its child.
pub(super) struct TileRecipe {
    /// Original application attachment handle.
    pub(super) record: maestro_web::SharedAttachment,
    /// Controlled later sibling failure.
    pub(super) fail: Rc<Cell<bool>>,
}
impl DomContent for TileRecipe {
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        let root = AttachmentTile::from(self.record.clone()).mount(view)?;
        self.update(&root, view)?;
        Ok(root)
    }
    fn update(&self, root: &DomRoot, view: &DomView) -> Result<(), ToolUiError> {
        AttachmentTile::from(self.record.clone()).update(root, view)?;
        if self.fail.get() {
            return Err(ToolUiError::Presentation(
                "controlled later child failure".into(),
            ));
        }
        Ok(())
    }
}
/// Both ordinary and rich header nesting use the same original tile.
#[derive(Clone, Copy)]
pub(super) enum Header {
    /// Direct renderer body.
    Plain,
    /// Rich header child.
    Rich,
}
/// Register a real tool callback retaining the original attachment and failure control.
pub(super) fn registration(name: &str, header: Header) -> Rc<Cell<bool>> {
    let flag = Rc::new(Cell::new(false));
    let fail = flag.clone();
    let record = crate::attachment_support::attachment();
    let renderer: Rc<dyn ToolRenderer> = Rc::new(recipes::Renderer(Rc::new(move |_, _, _| {
        let content: Box<dyn DomContent> = Box::new(TileRecipe {
            record: record.clone(),
            fail: fail.clone(),
        });
        let content = if matches!(header, Header::Rich) {
            render_header(
                ToolHeaderState::Complete,
                maestro_web::icons::icons().unwrap()["Code"].clone(),
                ToolHeaderText::Content(content),
            )
        } else {
            content
        };
        Ok(ToolRenderResult {
            content,
            is_custom: true,
        })
    })));
    maestro_web::register_tool_renderer(name, ToolRendererBinding::from_raw(renderer));
    flag
}
/// A genuine tile click opens an independent overlay before a later failed row refresh.
pub async fn registered(import: &js_sys::Function) {
    use crate::contributions_support::{container, host, select, text, view};
    let view = view();
    let _elements = host(&view, import);
    let downloads = crate::session_download_browser::Fixture::import(import).await;
    for header in [Header::Plain, Header::Rich] {
        let fail = registration("document-forwarding", header);
        let parent = container();
        let row = ToolMessage::<serde_json::Value> {
            tool_call: Some(Rc::new(serde_json::from_value(serde_json::json!({"type":"toolCall","id":"call","name":"document-forwarding","arguments":{}})).unwrap())),
            ..ToolMessage::new()
        };
        let mut slot = MountedContent::default();
        slot.show(&row, &parent, &view).unwrap();
        select(&parent, "[data-open]")
            .unchecked_ref::<web_sys::HtmlElement>()
            .click();
        let overlay = view
            .document
            .query_selector("attachment-overlay")
            .unwrap()
            .unwrap();
        fail.set(true);
        let error = slot.show(&row, &parent, &view).unwrap_err();
        assert!(
            matches!(error, ToolUiError::Presentation(ref message) if message == "controlled later child failure")
        );
        slot.clear();
        parent.remove();
        drop(row);
        assert!(overlay.is_connected());
        assert!(text(&overlay).contains("<literal>"));
        downloads
            .saved(
                select(&overlay, "[data-download]").unchecked_ref(),
                b"ABC",
                "text/plain",
                "note.txt",
            )
            .await;
        select(&overlay, "[data-close]")
            .unchecked_ref::<web_sys::HtmlElement>()
            .click();
        assert!(!overlay.is_connected());
        drop(
            maestro_web::tools::renderer_registry::TOOL_RENDERERS
                .with(|all| all.borrow_mut().shift_remove("document-forwarding")),
        );
    }
}
