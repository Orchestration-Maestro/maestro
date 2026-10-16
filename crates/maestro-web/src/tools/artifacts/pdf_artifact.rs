//! Owned PDF artifact with progressive native pages and original downloads.
use super::artifact_element::{ArtifactElement, ArtifactView};
use crate::{
    browser::attachment_content::{self, AttachmentHost, AttachmentUiError},
    components::download_button,
    dialogs::attachment_overlay::{error, pdf},
};
use std::rc::Rc;
/// A retained PDF view with independent original fields and document lifetime.
pub struct PdfArtifact {
    /// Literal authored filename.
    filename: String,
    /// Complete original encoded content.
    content: String,
    /// Caller platform effects.
    host: Rc<dyn AttachmentHost>,
    /// Stable native surface identity.
    id: egui::Id,
    /// Owned document and completed pages.
    pdf: pdf::Pdf,
    /// Body preparation awaits changed original content.
    changed: bool,
    /// Latched artifact load diagnostic.
    failure: Option<String>,
    /// Native error presentation, not error policy.
    error: error::Error,
    /// Retained repaint context, never borrowed drawing inputs.
    context: Option<egui::Context>,
    /// Whether the retained body is currently shown.
    visible: bool,
    /// Foreground layer containing the retained body.
    modal: Option<egui::Id>,
}
impl PdfArtifact {
    /// Start with empty fields using the supplied native host.
    #[must_use]
    pub fn new(host: Rc<dyn AttachmentHost>) -> Self {
        Self {
            filename: String::new(),
            content: String::new(),
            host,
            id: attachment_content::identity(),
            pdf: pdf::Pdf::default(),
            changed: false,
            failure: None,
            error: error::Error::default(),
            context: None,
            visible: true,
            modal: None,
        }
    }
    /// Wake the last header or body context after an owned edit.
    fn repaint(&self) {
        if let Some(context) = &self.context {
            context.request_repaint();
        }
    }
    /// Prepare changed bytes and complete this frame's single page.
    fn prepare(&mut self, ui: &egui::Ui) {
        let result = if self.changed {
            self.changed = false;
            if self.content.is_empty() {
                Ok(())
            } else {
                crate::utils::base64::original(&self.content)
                    .map_err(|e| e.to_string())
                    .and_then(|bytes| self.pdf.open(bytes))
            }
        } else {
            Ok(())
        };
        if let Err(message) = result.and_then(|()| self.pdf.advance(ui.ctx())) {
            self.failure = Some(message);
        }
    }
}
impl ArtifactElement for PdfArtifact {
    fn set_visibility(
        &mut self,
        visible: bool,
        modal: Option<egui::Id>,
    ) -> Result<(), AttachmentUiError> {
        self.host.set_retained_visibility(self.id, visible, modal)?;
        self.visible = visible;
        self.modal = modal;
        Ok(())
    }
    fn filename(&self) -> &str {
        &self.filename
    }
    fn set_filename(&mut self, filename: String) {
        self.filename = filename;
        self.repaint();
    }
    fn content(&self) -> &str {
        &self.content
    }
    fn set_content(&mut self, content: String) {
        if self.content == content && self.failure.is_none() {
            return;
        }
        let retain = content.is_empty() && self.failure.is_none();
        self.pdf.reset(retain);
        self.content = content;
        self.failure = None;
        self.changed = true;
        self.repaint();
    }
    fn get_header_buttons(
        &mut self,
        ui: &mut egui::Ui,
        view: ArtifactView<'_>,
    ) -> Result<(), AttachmentUiError> {
        self.context = Some(ui.ctx().clone());
        let bytes = crate::utils::base64::original(&self.content)?;
        download_button::draw(
            ui,
            view,
            self.host.as_ref(),
            &self.filename,
            ("application/pdf", &bytes),
        )
    }
    fn render(
        &mut self,
        ui: &mut egui::Ui,
        view: ArtifactView<'_>,
    ) -> Result<(), AttachmentUiError> {
        self.context = Some(ui.ctx().clone());
        if !self.visible {
            return Ok(());
        }
        self.prepare(ui);
        if let Some(message) = &self.failure {
            self.error.present(
                self.host.as_ref(),
                ui,
                (self.id, self.modal, true),
                (
                    [
                        view.i18n.text("Error loading PDF")?,
                        pdf::detail(message, view.i18n)?,
                    ],
                    view.presentation,
                ),
            )?;
        } else {
            let rect = ui
                .allocate_space(ui.available_size().max(egui::Vec2::ZERO))
                .1;
            self.pdf.present(
                self.host.as_ref(),
                ui,
                (self.id, rect, self.modal, true),
                view.presentation,
            )?;
        }
        Ok(())
    }
}
impl Drop for PdfArtifact {
    fn drop(&mut self) {
        self.host.remove(self.id);
    }
}
