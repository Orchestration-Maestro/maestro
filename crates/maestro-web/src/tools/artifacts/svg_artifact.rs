//! Retained svg document preview with original-source controls.
use super::artifact_element::{ArtifactElement, ArtifactView};
use super::image_artifact::ImageArtifact;
use super::text_artifact::{SourceProfile, TextArtifact};
use crate::tools::artifacts::ArtifactUiError;
use crate::{
    browser::attachment_content::AttachmentHost,
    components::preview_code_toggle::{PreviewCodeToggle, ViewMode},
};
use std::rc::Rc;
/// Editable document retaining its preview and source owners.
pub struct SvgArtifact {
    /// Original fields and common header/source owner.
    source: TextArtifact,
    /// Native preview owner.
    preview: ImageArtifact,
    /// Independent presentation selection.
    toggle: PreviewCodeToggle,
    /// Containing owner's current visibility.
    visible: bool,
    /// Containing modal identity.
    modal: Option<egui::Id>,
    /// Deferred platform failure for the infallible field setter.
    failure: Option<ArtifactUiError>,
    /// Most recently published native presentation.
    #[cfg(target_arch = "wasm32")]
    body: super::artifact_element::preview::LastBody,
    /// Retained native resource, independent of visible mode.
    #[cfg(target_arch = "wasm32")]
    preview_url: Option<crate::utils::download::ObjectUrl>,
}
impl SvgArtifact {
    /// Start with empty fields and preview selected using the supplied host.
    #[must_use]
    pub fn new(host: Rc<dyn AttachmentHost>) -> Self {
        Self {
            source: TextArtifact::with_profile(Rc::clone(&host), SourceProfile::Svg),
            preview: ImageArtifact::for_svg(host),
            toggle: PreviewCodeToggle::new(),
            visible: true,
            modal: None,
            failure: None,
            #[cfg(target_arch = "wasm32")]
            body: Rc::default(),
            #[cfg(target_arch = "wasm32")]
            preview_url: None,
        }
    }
    /// Route the selected branch through its existing retained owner.
    fn reconcile(&mut self) -> Result<(), ArtifactUiError> {
        if let Some(error) = self.failure.take() {
            return Err(error);
        }
        self.source.set_visibility(
            self.visible && self.toggle.mode() == ViewMode::Code,
            self.modal,
        )?;
        self.preview.set_visibility(
            self.visible && self.toggle.mode() == ViewMode::Preview,
            self.modal,
        )
    }
    /// Rebuild the native resource after an original-content edit or reconnection.
    #[cfg(target_arch = "wasm32")]
    fn update_preview_url(&mut self) -> Result<(), ArtifactUiError> {
        self.preview_url = None;
        self.preview_url = (!self.source.content().is_empty())
            .then(|| {
                crate::utils::download::ObjectUrl::new(
                    self.source.content().as_bytes(),
                    "image/svg+xml",
                )
            })
            .transpose()?;
        self.preview.set_content(
            self.preview_url
                .as_ref()
                .map_or_else(String::new, |url| url.as_str().into()),
        );
        Ok(())
    }
}
impl Default for SvgArtifact {
    /// Empty host-free SVG content, initially showing its preview.
    fn default() -> Self {
        Self {
            source: TextArtifact::page_profile(SourceProfile::Svg),
            preview: ImageArtifact::page_svg(),
            toggle: PreviewCodeToggle::new(),
            visible: true,
            modal: None,
            failure: None,
            #[cfg(target_arch = "wasm32")]
            body: Rc::default(),
            #[cfg(target_arch = "wasm32")]
            preview_url: None,
        }
    }
}
impl ArtifactElement for SvgArtifact {
    fn filename(&self) -> &str {
        self.source.filename()
    }
    fn content(&self) -> &str {
        self.source.content()
    }
    fn set_filename(&mut self, filename: String) {
        self.source.set_filename(filename.clone());
        self.preview.set_filename(filename);
    }
    fn set_content(&mut self, content: String) {
        if self.source.content() == content {
            return;
        }
        self.source.set_content(content);
        #[cfg(target_arch = "wasm32")]
        {
            if self.source.is_retained() {
                self.failure = self.update_preview_url().err();
            }
        }
    }
    fn set_visibility(
        &mut self,
        visible: bool,
        modal: Option<egui::Id>,
    ) -> Result<(), ArtifactUiError> {
        self.visible = visible;
        self.modal = modal;
        self.reconcile()
    }
    super::retained::artifact_header_buttons!();
    fn render(&mut self, ui: &mut egui::Ui, view: ArtifactView<'_>) -> Result<(), ArtifactUiError> {
        self.reconcile()?;
        if self.toggle.mode() == ViewMode::Code {
            self.source.render(ui, view)?;
        } else {
            self.preview.render(ui, view)?;
        }
        Ok(())
    }
    fn connected_callback(&mut self) -> Result<(), ArtifactUiError> {
        if let Some(error) = self.failure.take() {
            return Err(error);
        }
        #[cfg(target_arch = "wasm32")]
        if !self.source.content().is_empty() && self.preview_url.is_none() {
            self.update_preview_url()?;
        }
        Ok(())
    }
    fn disconnected_callback(&mut self) -> Result<(), ArtifactUiError> {
        #[cfg(target_arch = "wasm32")]
        {
            self.preview_url = None;
            self.preview.set_content(String::new());
        }
        self.failure.take().map_or(Ok(()), Err)
    }
}

#[cfg(target_arch = "wasm32")]
mod dom;
#[cfg(target_arch = "wasm32")]
pub(crate) use dom::define;
