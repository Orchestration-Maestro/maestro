//! Native mounting and event ownership.
use super::{Operand, UserMessage};
use crate::{
    browser::attachment_content::browser::{append, element},
    components::{attachment_tile::AttachmentTile, markdown_block::MarkdownBlock},
    dom::{self, DomContent, DomRoot, DomView},
    tools::types::ToolUiError,
};
use maestro_app::CustomAgentMessages;
use std::{cell::RefCell, sync::Arc};
use wasm_bindgen::JsCast as _;
use web_sys::Element;
/// Native children retained until the containing row replaces them.
type Published = RefCell<Vec<DomRoot>>;
impl<C: CustomAgentMessages + 'static> DomContent for UserMessage<C> {
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        dom::install_styles(view).map_err(|error| ToolUiError::native(&error))?;
        let root = DomRoot {
            node: element(&view.document, "div", "maestro-user-row")?.into(),
            owner: Some(Box::new(Published::default())),
        };
        self.update(&root, view)?;
        Ok(root)
    }
    fn update(&self, root: &DomRoot, view: &DomView) -> Result<(), ToolUiError> {
        let operand = self.operand()?;
        let published = root
            .owner
            .as_ref()
            .and_then(|owner| owner.downcast_ref::<Published>())
            .ok_or_else(|| ToolUiError::Presentation("Missing user row state".into()))?;
        let (bubble, shown) = prepare(&operand, view)?;
        let node: &Element = root.node.unchecked_ref();
        node.replace_children_with_node_1(&bubble);
        let previous = published.replace(shown);
        drop(previous);
        Ok(())
    }
}
/// Prepare every fallible child before publishing it into the containing row.
fn prepare(operand: &Operand, view: &DomView) -> Result<(Element, Vec<DomRoot>), ToolUiError> {
    let mut markdown = MarkdownBlock::default();
    markdown.content.clone_from(&operand.text);
    let markdown = markdown.mount(view)?;
    let tiles = operand
        .attachments
        .iter()
        .map(|attachment| AttachmentTile::from(Arc::clone(attachment)).mount(view))
        .collect::<Result<Vec<_>, _>>()?;
    let bubble = element(&view.document, "div", "maestro-user-bubble")?;
    append(&bubble, markdown.node.unchecked_ref())?;
    if !tiles.is_empty() {
        let container = element(&view.document, "div", "maestro-user-attachments")?;
        for tile in &tiles {
            append(&container, tile.node.unchecked_ref())?;
        }
        append(&bubble, &container)?;
    }
    let mut children = vec![markdown];
    children.extend(tiles);
    Ok((bubble, children))
}
