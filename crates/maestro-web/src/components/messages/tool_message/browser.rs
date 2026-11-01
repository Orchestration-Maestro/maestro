//! Native mounting and card placement.
use super::ToolMessage;
use crate::{
    browser::attachment_content::browser::element,
    components::messages::tool_message::dispatch,
    dom::{self, DomContent, DomRoot, DomView, MountedContent},
    tools::types::ToolUiError,
};
use std::cell::RefCell;
use wasm_bindgen::JsCast as _;
use web_sys::{Element, Node};

/// The published content and the ordinary card holding it, absent for custom content.
#[derive(Default)]
struct Placed {
    /// Ordinary card element, present only around default content.
    card: Option<Element>,
    /// Mounted renderer content.
    content: MountedContent,
}
impl<D: 'static> DomContent for ToolMessage<D> {
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        dom::install_styles(view).map_err(|error| ToolUiError::native(&error))?;
        let root = DomRoot {
            node: element(&view.document, "div", "maestro-tool-message")?.into(),
            owner: Some(Box::new(RefCell::new(Placed::default()))),
        };
        self.update(&root, view)?;
        Ok(root)
    }
    /// Update unchanged content in place; a changed card choice publishes a fully prepared replacement.
    fn update(&self, root: &DomRoot, view: &DomView) -> Result<(), ToolUiError> {
        let call = self
            .tool_call
            .as_deref()
            .ok_or_else(|| ToolUiError::Presentation("Missing tool call".into()))?;
        let rendered = dispatch(self, call, view)?;
        let placed = root
            .owner
            .as_ref()
            .and_then(|owner| owner.downcast_ref::<RefCell<Placed>>())
            .ok_or_else(|| ToolUiError::Presentation("Missing tool row state".into()))?;
        let row: &Element = root.node.unchecked_ref();
        let mut placed = placed.borrow_mut();
        let Placed { card, content } = &mut *placed;
        if content.root().is_some() && card.is_none() == rendered.is_custom {
            let parent: &Element = card.as_ref().unwrap_or(row);
            return content.show(rendered.content.as_ref(), parent, view);
        }
        let card = if rendered.is_custom {
            None
        } else {
            Some(element(&view.document, "div", "maestro-tool-message-card")?)
        };
        let top: Node = match &card {
            Some(card) => card.clone().into(),
            None => view.document.create_document_fragment().into(),
        };
        let mut content = MountedContent::default();
        content.show(rendered.content.as_ref(), &top, view)?;
        row.replace_children_with_node_1(&top);
        let previous = std::mem::replace(&mut *placed, Placed { card, content });
        drop(placed);
        drop(previous);
        Ok(())
    }
}
