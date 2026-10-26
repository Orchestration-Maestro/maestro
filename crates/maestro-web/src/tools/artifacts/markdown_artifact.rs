//! Retained markdown document preview with original-source controls.
use super::artifact_element::{ArtifactElement, ArtifactView};
use super::text_artifact::{SourceProfile, TextArtifact};
use crate::components::markdown_block::MarkdownBlock;
use crate::tools::artifacts::ArtifactUiError;
use crate::{
    browser::attachment_content::AttachmentHost,
    components::preview_code_toggle::{PreviewCodeToggle, ViewMode},
};
use std::rc::Rc;
/// Editable document retaining its preview and source owners.
pub struct MarkdownArtifact {
    /// Original fields and common header/source owner.
    source: TextArtifact,
    /// Native preview owner.
    preview: MarkdownBlock,
    /// Independent presentation selection.
    toggle: PreviewCodeToggle,
    /// Containing owner's current visibility.
    visible: bool,
    /// Containing modal identity.
    modal: Option<egui::Id>,
    /// Native body most recently mounted or updated.
    #[cfg(target_arch = "wasm32")]
    body: super::artifact_element::preview::LastBody,
}
impl MarkdownArtifact {
    /// Start with empty fields and preview selected using the supplied host.
    #[must_use]
    pub fn new(host: Rc<dyn AttachmentHost>) -> Self {
        Self {
            source: TextArtifact::with_profile(Rc::clone(&host), SourceProfile::Markdown),
            preview: MarkdownBlock::for_artifact(host),
            toggle: PreviewCodeToggle::new(),
            visible: true,
            modal: None,
            #[cfg(target_arch = "wasm32")]
            body: Rc::default(),
        }
    }
    /// Route the selected branch through its existing retained owner.
    fn reconcile(&mut self) -> Result<(), ArtifactUiError> {
        self.source.set_visibility(
            self.visible && self.toggle.mode() == ViewMode::Code,
            self.modal,
        )?;
        self.preview
            .set_visibility(
                self.visible && self.toggle.mode() == ViewMode::Preview,
                self.modal,
            )
            .map_err(ArtifactUiError::Host)
    }
}
impl Default for MarkdownArtifact {
    /// Empty host-free Markdown, initially showing its preview.
    fn default() -> Self {
        Self {
            source: TextArtifact::page_profile(SourceProfile::Markdown),
            preview: MarkdownBlock::default(),
            toggle: PreviewCodeToggle::new(),
            visible: true,
            modal: None,
            #[cfg(target_arch = "wasm32")]
            body: Rc::default(),
        }
    }
}
impl ArtifactElement for MarkdownArtifact {
    fn filename(&self) -> &str {
        self.source.filename()
    }
    fn content(&self) -> &str {
        self.source.content()
    }
    fn set_filename(&mut self, filename: String) {
        self.source.set_filename(filename);
    }
    fn set_content(&mut self, content: String) {
        self.preview.content.clone_from(&content);
        self.source.set_content(content);
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
            self.preview
                .render(
                    ui,
                    crate::components::code_block::CodeBlockView {
                        i18n: view.i18n,
                        presentation: view.presentation,
                        highlighter: view.highlighter,
                        modal: self.modal,
                    },
                )
                .map_err(ArtifactUiError::reading)?;
        }
        Ok(())
    }
}

#[cfg(target_arch = "wasm32")]
mod dom;
