//! Native mounting and event ownership.
use super::UserMessage;
use crate::{
    browser::attachment_content::browser::{append, element},
    components::{attachment_tile::AttachmentTile, markdown_block::MarkdownBlock},
    dom::{self, DomContent, DomRoot, DomView, MountedContent},
    tools::types::ToolUiError,
};
use maestro_app::CustomAgentMessages;
use std::cell::RefCell;
use web_sys::Element;
/// Bubble children mounted once and updated in place by position.
struct Placed {
    /// Bubble holding the text and, while it has tiles, the attachment row.
    bubble: Element,
    /// Markdown text root.
    text: MountedContent,
    /// Attachment row, detached while it has no tiles.
    attachments: Element,
    /// Tile roots by attachment position.
    tiles: Vec<MountedContent>,
}
impl<C: CustomAgentMessages + 'static> DomContent for UserMessage<C> {
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        dom::install_styles(view).map_err(|error| ToolUiError::native(&error))?;
        let node = element(&view.document, "div", "maestro-user-row")?;
        let bubble = element(&view.document, "div", "maestro-user-bubble")?;
        append(&node, &bubble)?;
        let placed = Placed {
            attachments: element(&view.document, "div", "maestro-user-attachments")?,
            bubble,
            text: MountedContent::default(),
            tiles: Vec::new(),
        };
        let root = DomRoot {
            node: node.into(),
            owner: Some(Box::new(RefCell::new(placed))),
        };
        self.update(&root, view)?;
        Ok(root)
    }
    /// Update the text and each tile in place, so unchanged children keep selection and focus.
    fn update(&self, root: &DomRoot, view: &DomView) -> Result<(), ToolUiError> {
        let operand = self.operand()?;
        let placed = root
            .owner
            .as_ref()
            .and_then(|owner| owner.downcast_ref::<RefCell<Placed>>())
            .ok_or_else(|| ToolUiError::Presentation("Missing user row state".into()))?;
        let mut placed = placed.borrow_mut();
        let Placed {
            bubble,
            text,
            attachments,
            tiles,
        } = &mut *placed;
        let mut markdown = MarkdownBlock::default();
        markdown.content = operand.text;
        text.show(&markdown, bubble, view)?;
        let count = operand.attachments.len();
        for (index, attachment) in operand.attachments.into_iter().enumerate() {
            if index == tiles.len() {
                tiles.push(MountedContent::default());
            }
            tiles[index].show(&AttachmentTile::from(attachment), attachments, view)?;
        }
        for tile in &mut tiles[count..] {
            tile.clear();
        }
        tiles.truncate(count);
        match (count, attachments.parent_node()) {
            (0, Some(_)) => attachments.remove(),
            (1.., None) => append(bubble, attachments)?,
            _ => {}
        }
        Ok(())
    }
}
