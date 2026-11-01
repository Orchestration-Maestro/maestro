//! Original-byte download control.
/// Native control operands and activation.
#[cfg(target_arch = "wasm32")]
pub mod dom;

use crate::browser::attachment_content::{self, AttachmentHost};
use crate::tools::artifacts::ArtifactUiError;
use crate::tools::artifacts::artifact_element::ArtifactView;
/// Original-byte control extent in logical points.
pub(crate) const SIZE: egui::Vec2 = egui::vec2(36., 32.);

/// Draw a small ghost control whose activation consumes only the supplied bytes.
pub(crate) fn draw(
    ui: &mut egui::Ui,
    view: ArtifactView<'_>,
    host: &dyn AttachmentHost,
    filename: &str,
    data: (&str, &[u8]),
) -> Result<(), ArtifactUiError> {
    draw_titled(ui, view, host, filename, (data.0, data.1, "Download"))
}
/// Draw the same byte control under the supplied localized title key.
pub(crate) fn draw_titled(
    ui: &mut egui::Ui,
    view: ArtifactView<'_>,
    host: &dyn AttachmentHost,
    filename: &str,
    data: (&str, &[u8], &str),
) -> Result<(), ArtifactUiError> {
    attachment_content::style(ui, view.presentation)
        .map_err(|error| ArtifactUiError::Presentation(error.to_string().into()))?;
    let title = view.i18n.text(data.2)?;
    let response = attachment_content::ghost_sized(ui, view.presentation, "Download", SIZE)?
        .on_hover_text(&title);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &title));
    if response.clicked() {
        host.download_bytes(data.1, filename, data.0)?;
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests;
