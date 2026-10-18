//! Owned spreadsheet artifact with native tables and original downloads.
use super::artifact_element::{ArtifactElement, ArtifactView};
use crate::tools::artifacts::ArtifactUiError;
use crate::{
    browser::attachment_content::{self, AttachmentHost},
    components::download_button,
    dialogs::attachment_overlay::{error, excel},
};
use std::rc::Rc;
/// A retained spreadsheet view with independent original fields and workbook lifetime.
pub struct ExcelArtifact {
    /// Literal authored filename.
    filename: String,
    /// Complete original encoded content.
    content: String,
    /// Caller platform effects.
    host: Rc<dyn AttachmentHost>,
    /// Stable native surface identity.
    id: egui::Id,
    /// Owned prepared tables.
    excel: excel::Excel,
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
}
impl ExcelArtifact {
    /// Start with empty fields using the supplied native host.
    #[must_use]
    pub fn new(host: Rc<dyn AttachmentHost>) -> Self {
        Self {
            filename: String::new(),
            content: String::new(),
            host,
            id: attachment_content::identity(),
            excel: excel::Excel::default(),
            changed: false,
            failure: None,
            error: error::Error::default(),
            context: None,
            visible: true,
        }
    }
    /// Wake the last header or body context after an owned edit.
    fn repaint(&self) {
        if let Some(context) = &self.context {
            context.request_repaint();
        }
    }
    /// Prepare changed bytes and complete this frame's workbook.
    fn prepare(&mut self) {
        let result = if self.changed {
            self.changed = false;
            if self.content.is_empty() {
                Ok(())
            } else {
                crate::utils::base64::original(&self.content)
                    .map_err(|e| e.to_string())
                    .and_then(|bytes| self.excel.open(&bytes))
            }
        } else {
            Ok(())
        };
        if let Err(message) = result {
            self.failure = Some(message);
        }
    }
}
impl ArtifactElement for ExcelArtifact {
    fn set_visibility(
        &mut self,
        visible: bool,
        modal: Option<egui::Id>,
    ) -> Result<(), ArtifactUiError> {
        self.host.set_retained_visibility(self.id, visible, modal)?;
        self.visible = visible;
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
        self.excel.reset(retain);
        self.content = content;
        self.failure = None;
        self.changed = true;
        self.repaint();
    }
    fn get_header_buttons(
        &mut self,
        ui: &mut egui::Ui,
        view: ArtifactView<'_>,
    ) -> Result<(), ArtifactUiError> {
        self.context = Some(ui.ctx().clone());
        let bytes = crate::utils::base64::original(&self.content)
            .map_err(|error| ArtifactUiError::Presentation(error.to_string().into()))?;
        download_button::draw(
            ui,
            view,
            self.host.as_ref(),
            &self.filename,
            (
                if super::extension(&self.filename) == "xls" {
                    "application/vnd.ms-excel"
                } else {
                    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
                },
                &bytes,
            ),
        )
    }
    fn render(&mut self, ui: &mut egui::Ui, view: ArtifactView<'_>) -> Result<(), ArtifactUiError> {
        self.context = Some(ui.ctx().clone());
        if !self.visible {
            return Ok(());
        }
        self.prepare();
        if let Some(message) = &self.failure {
            self.error.present(
                self.host.as_ref(),
                ui,
                (self.id, None, true),
                (
                    [
                        view.i18n.text("Error loading spreadsheet")?,
                        error::detail(message, view.i18n, "Failed to load spreadsheet")?,
                    ],
                    view.presentation,
                ),
            )?;
        } else {
            let rect = ui
                .allocate_space(ui.available_size().max(egui::Vec2::ZERO))
                .1;
            self.excel.present(
                self.host.as_ref(),
                ui,
                (self.id, rect, None, true),
                (
                    view.presentation,
                    &view.i18n.text("Error loading spreadsheet")?,
                ),
            )?;
        }
        Ok(())
    }
}
impl Drop for ExcelArtifact {
    fn drop(&mut self) {
        self.host.remove(self.id);
    }
}
