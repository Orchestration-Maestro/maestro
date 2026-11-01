//! Word document artifact with independent native body and download lifetimes.
use crate::{
    dom::{DomContent, DomRoot, DomView},
    tools::{artifacts::artifact_element::header, types::ToolUiError},
};
use std::{cell::RefCell, rc::Rc};

mod browser;

/// Original Word content whose mounted bodies retain their document resources.
#[derive(Default)]
pub struct DocxArtifact {
    /// Authored download filename.
    pub filename: String,
    /// Complete original encoded document.
    content: String,
    /// Assignment identity distinguishes a requested failed retry from an unrelated update.
    assignment: Rc<()>,
}
impl DocxArtifact {
    /// Start with empty filename and content.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    /// Borrow the original encoded document.
    #[must_use]
    pub fn content(&self) -> &str {
        &self.content
    }
    /// Replace the original document; an equal failed assignment requests another attempt.
    pub fn set_content(&mut self, content: String) {
        self.content = content;
        self.assignment = Rc::new(());
    }
    /// Build an independent download header from complete original bytes.
    /// # Errors
    /// Returns decoding, localization or native control failures.
    pub fn get_header_buttons(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        header::download(
            view,
            &self.filename,
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            &self.bytes()?,
        )
    }
    /// Decode the complete original payload through the shared byte owner.
    fn bytes(&self) -> Result<Vec<u8>, ToolUiError> {
        crate::utils::base64::original(&self.content)
            .map_err(|error| ToolUiError::Presentation(error.to_string()))
    }
}
impl DomContent for DocxArtifact {
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        let node = view
            .document
            .create_element("section")
            .map_err(|error| ToolUiError::native(&error))?;
        node.set_class_name("maestro-docx-artifact");
        let root = DomRoot {
            node: node.into(),
            owner: Some(Box::new(RefCell::new(browser::Mounted::default()))),
        };
        self.update(&root, view)?;
        Ok(root)
    }
    fn update(&self, root: &DomRoot, view: &DomView) -> Result<(), ToolUiError> {
        let state = root
            .owner
            .as_ref()
            .and_then(|owner| owner.downcast_ref::<RefCell<browser::Mounted>>())
            .ok_or_else(|| ToolUiError::Presentation("Word body is not mounted".to_owned()))?;
        browser::update(self, root, view, &mut state.borrow_mut())
    }
}
