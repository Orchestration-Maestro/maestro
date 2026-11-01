//! Native image artifact with original-byte download and branded load fallback.
use super::artifact_element::{ArtifactElement, ArtifactView};
use crate::tools::artifacts::ArtifactUiError;
use crate::{
    browser::attachment_content::{self, AttachmentHost, ReadingContent},
    components::download_button,
};
use std::{cell::RefCell, rc::Rc};
/// Current image operands, independent from the element's possibly failed source.
#[derive(Default)]
struct Body {
    /// Authored source URL last prepared.
    source: String,
    /// Literal alternative text.
    alt: String,
    /// Selected pack's fallback image.
    fallback: String,
    /// Whether the current source failed to load.
    failed: bool,
    /// Literal SVG URL with native failure presentation.
    svg: bool,
    /// Mounted image node.
    #[cfg(target_arch = "wasm32")]
    image: Option<web_sys::Element>,
}
/// Editable image artifact retaining native pointer interaction and load ownership.
pub struct ImageArtifact {
    /// Authored filename.
    filename: String,
    /// Original base64 or data URL.
    content: String,
    /// Platform effects.
    host: Option<Rc<dyn AttachmentHost>>,
    /// Independent surface identity.
    id: egui::Id,
    /// Live native operands.
    body: Rc<RefCell<Body>>,
    /// Successfully published factory.
    reading: Option<Rc<ReadingContent>>,
    /// Last selected fallback colour.
    muted: String,
    /// Whether authored operands changed.
    changed: bool,
    /// Last drawing context.
    context: Option<egui::Context>,
}
impl ImageArtifact {
    /// Start with empty filename and content using the supplied native host.
    #[must_use]
    pub fn new(host: Rc<dyn AttachmentHost>) -> Self {
        Self::with_host(Some(host))
    }
    /// Create image storage independently of retained effects.
    fn with_host(host: Option<Rc<dyn AttachmentHost>>) -> Self {
        Self {
            filename: String::new(),
            content: String::new(),
            host,
            id: attachment_content::identity(),
            body: Rc::default(),
            reading: None,
            muted: String::new(),
            changed: true,
            context: None,
        }
    }
    /// Create a host-free literal SVG image with no fallback substitution.
    pub(super) fn page_svg() -> Self {
        let image = Self::default();
        image.body.borrow_mut().svg = true;
        image
    }
    /// Reuse the native image owner for a literal SVG resource.
    pub(super) fn for_svg(host: Rc<dyn AttachmentHost>) -> Self {
        let artifact = Self::new(host);
        artifact.body.borrow_mut().svg = true;
        artifact
    }
    /// Wake the last frame after edits.
    fn repaint(&self) {
        if let Some(context) = &self.context {
            context.request_repaint();
        }
    }
    /// Select image MIME by last filename segment.
    fn mime(&self) -> &'static str {
        match super::extension(&self.filename).as_str() {
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "webp" => "image/webp",
            "svg" => "image/svg+xml",
            "bmp" => "image/bmp",
            "ico" => "image/x-icon",
            _ => "image/png",
        }
    }
    /// Decode only the header's download branch, never the display body.
    fn bytes(&self) -> Result<Vec<u8>, ArtifactUiError> {
        let payload = if self.content.starts_with("data:") {
            crate::utils::base64::data_payload(&self.content).unwrap_or("")
        } else {
            &self.content
        };
        crate::utils::base64::decode(payload)
            .map_err(|error| ArtifactUiError::Presentation(error.to_string().into()))
    }
    /// Recompute image operands only after an edit or selected-pack change.
    fn prepare(&mut self, view: ArtifactView<'_>) {
        let muted = view
            .presentation
            .colors
            .get("muted")
            .copied()
            .unwrap_or_default();
        if !self.changed && muted == self.muted {
            return;
        }
        self.assign(&mut self.body.borrow_mut(), muted);
        muted.clone_into(&mut self.muted);
        self.changed = false;
    }
    /// Refresh source and fallback while retaining a failed unchanged image.
    fn assign(&self, body: &mut Body, muted: &str) {
        let source = if body.svg || self.content.starts_with("data:") {
            self.content.clone()
        } else {
            format!("data:{};base64,{}", self.mime(), self.content)
        };
        if body.source != source {
            body.failed = false;
        }
        body.source = source;
        body.alt.clone_from(&self.filename);
        body.fallback = format!(
            "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 100 100'%3E%3Ctext x='50' y='50' text-anchor='middle' dominant-baseline='middle' fill='%23{}'%3EImage Error%3C/text%3E%3C/svg%3E",
            muted.trim_start_matches('#')
        );
    }
    super::retained::artifact_reading!(body);
}
impl Default for ImageArtifact {
    /// Empty host-free image content.
    fn default() -> Self {
        Self::with_host(None)
    }
}
impl ArtifactElement for ImageArtifact {
    fn filename(&self) -> &str {
        &self.filename
    }
    fn content(&self) -> &str {
        &self.content
    }
    fn set_filename(&mut self, filename: String) {
        self.changed |= self.filename != filename;
        self.filename = filename;
        self.repaint();
    }
    fn set_content(&mut self, content: String) {
        self.changed |= self.content != content;
        self.content = content;
        self.repaint();
    }
    super::retained::artifact_visibility!();
    fn get_header_buttons(
        &mut self,
        ui: &mut egui::Ui,
        view: ArtifactView<'_>,
    ) -> Result<(), ArtifactUiError> {
        self.context = Some(ui.ctx().clone());
        download_button::draw(
            ui,
            view,
            super::retained::host(self.host.as_ref())?.as_ref(),
            &self.filename,
            (self.mime(), &self.bytes()?),
        )
    }
    fn render(&mut self, ui: &mut egui::Ui, view: ArtifactView<'_>) -> Result<(), ArtifactUiError> {
        self.context = Some(ui.ctx().clone());
        self.prepare(view);
        #[cfg(target_arch = "wasm32")]
        browser::sync(&self.body.borrow())
            .map_err(|error| attachment_content::browser::host_error(&error))?;
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
impl Drop for ImageArtifact {
    fn drop(&mut self) {
        if let Some(host) = &self.host {
            host.remove(self.id);
        }
    }
}
#[cfg(target_arch = "wasm32")]
mod browser;

#[cfg(target_arch = "wasm32")]
mod dom;
