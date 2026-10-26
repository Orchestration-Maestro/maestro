//! Refresh original record fields without restarting unchanged native documents.
use super::State;
use crate::{dom, tools::types::ToolUiError};
use std::rc::Rc;

impl State {
    /// Refresh chrome and changed content while preserving the containing element and error latch.
    pub(super) fn refresh(self: &Rc<Self>) -> Result<(), ToolUiError> {
        if self.closed.get() {
            return Ok(());
        }
        let (name, changed) = {
            let attachment = self
                .attachment
                .try_read()
                .map_err(|_| ToolUiError::Presentation("Attachment borrow conflict".into()))?;
            let changed = self
                .previous
                .borrow()
                .as_ref()
                .is_none_or(|previous| !previous.matches(&attachment, self.extracted.get()));
            (attachment.file_name.clone(), changed)
        };
        let options = self.mode_options()?;
        let root = self
            .root
            .borrow()
            .as_ref()
            .and_then(dom::live)
            .ok_or_else(|| ToolUiError::Presentation("Missing overlay root".into()))?;
        if let Some(label) = root
            .query_selector(".maestro-attachment-name")
            .map_err(|error| ToolUiError::native(&error))?
        {
            label.set_text_content(Some(&name));
        }
        if let Some(modes) = self.modes.borrow_mut().as_mut() {
            modes.show(&self.view, options)?;
        }
        if changed && !self.failed.get() {
            self.show_content()?;
        }
        Ok(())
    }
}
