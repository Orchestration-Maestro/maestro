//! Owned artifact fields with independent header and body drawing.
#[cfg(target_arch = "wasm32")]
pub(crate) mod header;
#[cfg(target_arch = "wasm32")]
pub(crate) mod preview;
use crate::tools::artifacts::ArtifactUiError;
/// Call-scoped localization and branding, never retained by a native factory.
#[derive(Clone, Copy)]
pub struct ArtifactView<'a> {
    /// Classifies code for this frame, never retained by the view.
    pub highlighter: &'a dyn maestro_theme::SyntaxHighlighter,
    /// Caller localization.
    pub i18n: &'a crate::I18n,
    /// Caller brand projection.
    pub presentation: &'a maestro_theme::BrandPresentation<'a>,
}
/// An editable artifact with its independent header controls and native body.
pub trait ArtifactElement {
    /// Read the authored filename.
    fn filename(&self) -> &str;
    /// Replace the authored filename.
    fn set_filename(&mut self, filename: String);
    /// Read the original content.
    fn content(&self) -> &str;
    /// Replace the original content.
    fn set_content(&mut self, content: String);
    /// Change retained visibility and containing modal without releasing the view.
    /// # Errors
    /// Returns native retained-policy failures.
    fn set_visibility(
        &mut self,
        visible: bool,
        modal: Option<egui::Id>,
    ) -> Result<(), ArtifactUiError>;
    /// Draw independent header controls once per toolkit render pass.
    /// Layout may require multiple passes within one presentation.
    /// # Errors
    /// Returns localization, decoding or platform failures.
    fn get_header_buttons(
        &mut self,
        ui: &mut egui::Ui,
        view: ArtifactView<'_>,
    ) -> Result<(), ArtifactUiError>;
    /// Draw the body into the supplied viewport.
    /// # Errors
    /// Returns localization or native placement failures.
    fn render(&mut self, ui: &mut egui::Ui, view: ArtifactView<'_>) -> Result<(), ArtifactUiError>;
    /// Attach native resources without changing visibility.
    /// # Errors
    /// Returns native resource creation failures.
    fn connected_callback(&mut self) -> Result<(), ArtifactUiError> {
        Ok(())
    }
    /// Release attachment-scoped resources, retaining authored fields.
    /// # Errors
    /// Returns native resource release failures.
    fn disconnected_callback(&mut self) -> Result<(), ArtifactUiError> {
        Ok(())
    }
}

/// Synchronize a source tag's reflected filename without repeating attribute callbacks.
#[cfg(target_arch = "wasm32")]
pub(crate) fn filename(
    node: &web_sys::Element,
    filename: &str,
) -> Result<(), crate::tools::types::ToolUiError> {
    if node.get_attribute("filename").as_deref() != Some(filename) {
        node.set_attribute("filename", filename)
            .map_err(|error| crate::tools::types::ToolUiError::native(&error))?;
    }
    Ok(())
}
