//! Download-only artifact with a retained literal native reading body.
use super::artifact_element::{ArtifactElement, ArtifactView};
use crate::tools::artifacts::ArtifactUiError;
use crate::{
    browser::attachment_content::{self, AttachmentHost, ReadingContent},
    components::download_button,
};
use std::{cell::RefCell, rc::Rc};
/// Current owned native text and, in a browser, its retained nodes.
#[derive(Default)]
struct Text {
    /// Filename followed by the two localized guidance sentences.
    values: [String; 3],
    /// Mounted nodes survive field edits without replacing the root.
    #[cfg(target_arch = "wasm32")]
    nodes: Option<[web_sys::Element; 3]>,
}
/// Generic file with independent original content and native reading lifetime.
pub struct GenericArtifact {
    /// Original authored filename.
    filename: String,
    /// Original base64 content, never decoded by body drawing.
    content: String,
    /// Replaceable native effects.
    host: Option<Rc<dyn AttachmentHost>>,
    /// Surface identity independent of the filename.
    id: egui::Id,
    /// Owned current native operands.
    text: Rc<RefCell<Text>>,
    /// Successfully published factory identity.
    reading: Option<Rc<ReadingContent>>,
    /// Owned repaint handle retained after drawing.
    context: Option<egui::Context>,
}
impl GenericArtifact {
    /// Start with empty filename and content using the supplied native host.
    #[must_use]
    pub fn new(host: Rc<dyn AttachmentHost>) -> Self {
        Self::with_host(Some(host))
    }
    /// Create file storage independently of retained effects.
    fn with_host(host: Option<Rc<dyn AttachmentHost>>) -> Self {
        Self {
            filename: String::new(),
            content: String::new(),
            host,
            id: attachment_content::identity(),
            text: Rc::new(RefCell::new(Text::default())),
            reading: None,
            context: None,
        }
    }
    /// Wake the last drawing context without retaining borrowed drawing inputs.
    fn repaint(&self) {
        if let Some(context) = &self.context {
            context.request_repaint();
        }
    }
    /// Select complete original bytes, including folded data URL payloads.
    fn bytes(&self) -> Result<Vec<u8>, ArtifactUiError> {
        crate::utils::base64::original(&self.content)
            .map_err(|error| ArtifactUiError::Presentation(error.to_string().into()))
    }
    /// Select the explicitly supported download media types.
    fn mime(&self) -> &'static str {
        match super::extension(&self.filename).as_str() {
            "pdf" => "application/pdf",
            "zip" => "application/zip",
            "tar" => "application/x-tar",
            "gz" => "application/gzip",
            "rar" => "application/vnd.rar",
            "7z" => "application/x-7z-compressed",
            "mp3" => "audio/mpeg",
            "mp4" => "video/mp4",
            "avi" => "video/x-msvideo",
            "mov" => "video/quicktime",
            "wav" => "audio/wav",
            "ogg" => "audio/ogg",
            "json" => "application/json",
            "xml" => "application/xml",
            _ => "application/octet-stream",
        }
    }
    /// Filename followed by the two localized guidance sentences.
    fn text_values(&self, i18n: &crate::I18n) -> Result<[String; 3], crate::I18nError> {
        Ok([
            self.filename.clone(),
            i18n.text("Preview not available for this file type.")?,
            i18n.text("Click the download button above to view it on your computer.")?,
        ])
    }
    /// Update owned text operands and only the changed native nodes.
    fn update_text(&self, view: ArtifactView<'_>) -> Result<(), crate::I18nError> {
        self.text.borrow_mut().values = self.text_values(view.i18n)?;
        #[cfg(target_arch = "wasm32")]
        {
            let text = self.text.borrow();
            crate::browser::attachment_content::browser::sync_text(
                text.nodes.as_ref(),
                &text.values,
            );
        }
        Ok(())
    }
    super::retained::artifact_reading!(text);
}
impl Drop for GenericArtifact {
    fn drop(&mut self) {
        if let Some(host) = &self.host {
            host.remove(self.id);
        }
    }
}
impl Default for GenericArtifact {
    /// Empty host-free generic file content.
    fn default() -> Self {
        Self::with_host(None)
    }
}
impl ArtifactElement for GenericArtifact {
    super::retained::artifact_visibility!();
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
        self.content = content;
        self.repaint();
    }
    fn get_header_buttons(
        &mut self,
        ui: &mut egui::Ui,
        view: ArtifactView<'_>,
    ) -> Result<(), ArtifactUiError> {
        self.context = Some(ui.ctx().clone());
        let bytes = self.bytes()?;
        ui.scope(|ui| {
            download_button::draw(
                ui,
                view,
                super::retained::host(self.host.as_ref())?.as_ref(),
                &self.filename,
                (self.mime(), &bytes),
            )
        })
        .inner
    }
    fn render(&mut self, ui: &mut egui::Ui, view: ArtifactView<'_>) -> Result<(), ArtifactUiError> {
        self.context = Some(ui.ctx().clone());
        self.update_text(view)?;
        let reading = self.reading();
        let rect = ui
            .allocate_space(ui.available_size().max(egui::Vec2::ZERO))
            .1;
        attachment_content::reading::present(
            super::retained::host(self.host.as_ref())?.as_ref(),
            ui,
            (self.id, rect, None),
            &reading,
            view.presentation,
        )?;
        self.reading = Some(reading);
        Ok(())
    }
}
#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(target_arch = "wasm32")]
mod dom;
