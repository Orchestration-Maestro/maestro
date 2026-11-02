//! The list root and one placement per emitted position.
use super::MessageList;
use crate::{
    browser::attachment_content::browser::element,
    dom::{self, DomContent, DomRoot, DomView, MountedContent},
    tools::types::ToolUiError,
};
use maestro_app::CustomAgentMessages;
use std::cell::RefCell;

/// Row placements, the first one for emitted position 0.
type Placements = RefCell<Vec<MountedContent>>;

impl<C: CustomAgentMessages + 'static> DomContent for MessageList<C> {
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        dom::install_styles(view).map_err(|error| ToolUiError::native(&error))?;
        let root = DomRoot {
            node: element(&view.document, "div", "maestro-message-list")?.into(),
            owner: Some(Box::new(Placements::default())),
        };
        self.update(&root, view)?;
        Ok(root)
    }
    /// Prepare every row first, then update each position in place, add new positions at the end
    /// and remove positions past the last row.
    fn update(&self, root: &DomRoot, view: &DomView) -> Result<(), ToolUiError> {
        let rows = self.prepare()?;
        let placements = root
            .owner
            .as_ref()
            .and_then(|owner| owner.downcast_ref::<Placements>())
            .ok_or_else(|| ToolUiError::Presentation("Missing list state".into()))?;
        let mut placed = placements.take();
        placed.resize_with(placed.len().max(rows.len()), MountedContent::default);
        let result = rows
            .iter()
            .zip(&mut placed)
            .try_for_each(|(row, placement)| placement.show(row.as_ref(), &root.node, view));
        for mut retired in placed.split_off(rows.len()) {
            retired.clear();
        }
        let old = placements.replace(placed);
        drop(old);
        result
    }
}
