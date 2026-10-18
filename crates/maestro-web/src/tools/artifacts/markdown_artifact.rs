//! Retained markdown document preview with original-source controls.
use super::artifact_element::{ArtifactElement, ArtifactView};
use super::text_artifact::{SourceProfile, TextArtifact};
use crate::components::markdown_block::MarkdownBlock;
use crate::{
    browser::attachment_content::{AttachmentHost, AttachmentUiError},
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
        }
    }
    /// Route the selected branch through its existing retained owner.
    fn reconcile(&mut self) -> Result<(), AttachmentUiError> {
        self.source.set_visibility(
            self.visible && self.toggle.mode == ViewMode::Code,
            self.modal,
        )?;
        self.preview.set_visibility(
            self.visible && self.toggle.mode == ViewMode::Preview,
            self.modal,
        )
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
    ) -> Result<(), AttachmentUiError> {
        self.visible = visible;
        self.modal = modal;
        self.reconcile()
    }
    fn get_header_buttons(
        &mut self,
        ui: &mut egui::Ui,
        view: ArtifactView<'_>,
    ) -> Result<(), AttachmentUiError> {
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.x =
                crate::browser::attachment_content::points(view.presentation.spacing["sm"]);
            self.toggle.render(ui, view.i18n, view.presentation)?;
            self.reconcile()?;
            self.source.get_header_buttons(ui, view)
        })
        .inner
    }
    fn render(
        &mut self,
        ui: &mut egui::Ui,
        view: ArtifactView<'_>,
    ) -> Result<(), AttachmentUiError> {
        self.reconcile()?;
        if self.toggle.mode == ViewMode::Code {
            self.source.render(ui, view)?;
        } else {
            self.preview.render(
                ui,
                crate::components::code_block::CodeBlockView {
                    i18n: view.i18n,
                    presentation: view.presentation,
                    highlighter: view.highlighter,
                    modal: self.modal,
                },
            )?;
        }
        Ok(())
    }
}
