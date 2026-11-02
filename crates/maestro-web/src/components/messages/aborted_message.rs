//! Localized literal abort row.
#[cfg(target_arch = "wasm32")]
use crate::{
    browser::attachment_content::browser::{element, set_text_if_changed},
    dom::{DomContent, DomRoot, DomView, install_styles},
    tools::types::ToolUiError,
};

/// Localized literal abort text as page content.
#[derive(Default)]
pub struct AbortedMessage;
#[cfg(target_arch = "wasm32")]
impl DomContent for AbortedMessage {
    fn mount(&self, view: &DomView) -> Result<DomRoot, ToolUiError> {
        install_styles(view).map_err(|error| ToolUiError::native(&error))?;
        let root = DomRoot::new(element(&view.document, "span", "maestro-message-abort")?);
        self.update(&root, view)?;
        Ok(root)
    }
    fn update(&self, root: &DomRoot, view: &DomView) -> Result<(), ToolUiError> {
        use wasm_bindgen::JsCast as _;
        set_text_if_changed(
            root.node.unchecked_ref(),
            &view.i18n.text("Request aborted")?,
        );
        Ok(())
    }
}
