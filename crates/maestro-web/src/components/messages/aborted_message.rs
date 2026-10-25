//! Localized literal abort row.
use crate::{
    browser::attachment_content::AttachmentHost,
    components::native_text::{NativeText, TextKind},
    tools::types::{ToolRenderView, ToolUiError},
};
use std::rc::Rc;

/// Retained localized abort text.
pub struct AbortedMessage {
    /// Literal native reading lifetime.
    text: NativeText,
}
impl AbortedMessage {
    /// Bind abort text to the supplied platform.
    #[must_use]
    pub fn new(host: Rc<dyn AttachmentHost>) -> Self {
        Self {
            text: NativeText::new(host),
        }
    }
    /// Draw the localized italic abort text.
    /// # Errors
    /// Returns reached localization or native publication failures.
    pub fn render(
        &mut self,
        ui: &mut egui::Ui,
        view: ToolRenderView<'_>,
    ) -> Result<egui::Response, ToolUiError> {
        self.text.render(
            ui,
            (&view.i18n.text("Request aborted")?, TextKind::Abort),
            view,
        )
    }
}
