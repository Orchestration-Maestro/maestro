//! Native page body and independent header of the generic file artifact.
use super::GenericArtifact;
use crate::{
    browser::attachment_content::browser::sync_text,
    dom::{DomContent, DomRoot, DomView},
    tools::{artifacts::artifact_element::header, types::ToolUiError},
};
use web_sys::Element;

/// Text nodes owned by one mounted body.
struct Mounted {
    /// Filename node followed by the two guidance nodes.
    text: [Element; 3],
}
impl GenericArtifact {
    /// Build this file type's independent native header.
    /// # Errors
    /// Returns decoding, localization or native failures.
    pub fn get_header_buttons(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        header::download(view, &self.filename, self.mime(), &self.bytes()?)
    }
}
impl DomContent for GenericArtifact {
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        let (node, text) = super::browser::body(&view.document).map_err(ToolUiError::Host)?;
        let root = DomRoot {
            node: node.into(),
            owner: Some(Box::new(Mounted { text })),
        };
        self.update(&root, view)?;
        Ok(root)
    }
    fn update(&self, root: &DomRoot, view: &DomView) -> Result<(), ToolUiError> {
        let mounted = root
            .owner
            .as_ref()
            .and_then(|owner| owner.downcast_ref::<Mounted>())
            .ok_or_else(|| ToolUiError::Presentation("Generic body is not mounted".to_owned()))?;
        let values = self.text_values(&view.i18n).map_err(ToolUiError::I18n)?;
        sync_text(Some(&mounted.text), &values);
        Ok(())
    }
}
