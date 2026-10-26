//! Independent Markdown preview and source placements with shared controls.
use super::MarkdownArtifact;
use crate::{
    dom::{DomContent, DomRoot, DomView},
    tools::{
        artifacts::artifact_element::preview::{self, Regions},
        types::ToolUiError,
    },
};
use std::rc::Rc;

impl MarkdownArtifact {
    /// Build preview/source, copy and original UTF-8 download controls.
    /// # Errors
    /// Returns localization or native control failures.
    pub fn get_header_buttons(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        preview::header(
            view,
            self.source.get_header_buttons(view)?,
            &self.toggle,
            &self.body,
        )
    }
}
impl DomContent for MarkdownArtifact {
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        let body = Regions::mount(view, &self.preview, &self.source, &self.toggle)?;
        body.show(self.toggle.mode())?;
        preview::publish(&self.body, &body);
        body.bind(&self.toggle, view)?;
        Ok(DomRoot {
            node: body.node.clone().into(),
            owner: Some(Box::new(body)),
        })
    }
    fn update(&self, root: &DomRoot, view: &DomView) -> Result<(), ToolUiError> {
        let body = root
            .owner
            .as_ref()
            .and_then(|owner| owner.downcast_ref::<Rc<Regions>>())
            .ok_or_else(|| ToolUiError::Presentation("Markdown body is not mounted".into()))?;
        preview::publish(&self.body, body);
        body.bind(&self.toggle, view)?;
        body.update(view, &self.preview, &self.source)?;
        body.show(self.toggle.mode())
    }
}
