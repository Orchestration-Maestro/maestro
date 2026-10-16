//! Original-byte download control.
use crate::browser::attachment_content::{self, AttachmentHost, AttachmentUiError};
use crate::tools::artifacts::artifact_element::ArtifactView;
/// Draw a small ghost control whose activation consumes only the supplied bytes.
pub(crate) fn draw(
    ui: &mut egui::Ui,
    view: ArtifactView<'_>,
    host: &dyn AttachmentHost,
    filename: &str,
    data: (&str, &[u8]),
) -> Result<(), AttachmentUiError> {
    attachment_content::style(ui, view.presentation)?;
    let title = view.i18n.text("Download")?;
    let response =
        attachment_content::ghost_sized(ui, view.presentation, "Download", egui::vec2(36., 32.))?
            .on_hover_text(&title);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &title));
    if response.clicked() {
        host.download_bytes(data.1, filename, data.0)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
