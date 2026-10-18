//! Retained selection between a document preview and its original source.
use crate::{
    I18n,
    browser::attachment_content::{self, AttachmentUiError},
};
/// The two supported artifact presentations.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ViewMode {
    /// Render document content.
    #[default]
    Preview,
    /// Show original source.
    Code,
}
/// An independently retained pair of preview/source controls.
pub struct PreviewCodeToggle {
    /// Current selected presentation.
    pub mode: ViewMode,
    /// Per-instance toolkit identity.
    id: egui::Id,
}
impl Default for PreviewCodeToggle {
    fn default() -> Self {
        Self::new()
    }
}
impl PreviewCodeToggle {
    /// Start with preview selected.
    #[must_use]
    pub fn new() -> Self {
        Self {
            mode: ViewMode::Preview,
            id: attachment_content::identity(),
        }
    }
    /// Draw both controls, returning only a changed user selection.
    /// # Errors
    /// Returns localization, glyph or brand projection failures.
    pub fn render<F: ?Sized>(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &I18n<F>,
        presentation: &maestro_theme::BrandPresentation<'_>,
    ) -> Result<Option<ViewMode>, AttachmentUiError> {
        use attachment_content::{color, points};
        attachment_content::style(ui, presentation)?;
        let radius = egui::CornerRadius::from(points(presentation.radii["control"]));
        ui.push_id(self.id, |ui| {
            egui::Frame::new()
                .fill(color(presentation, "panel")?.gamma_multiply(0.6))
                .stroke(egui::Stroke::new(1., color(presentation, "border")?))
                .corner_radius(radius)
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.x = 0.;
                    ui.horizontal(|ui| self.controls(ui, i18n, presentation))
                        .inner
                })
                .inner
        })
        .inner
    }
    /// Draw the two adjacent controls inside their shared group.
    fn controls<F: ?Sized>(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &I18n<F>,
        presentation: &maestro_theme::BrandPresentation<'_>,
    ) -> Result<Option<ViewMode>, AttachmentUiError> {
        let mut transition = None;
        for (mode, title, glyph) in [
            (ViewMode::Preview, "Preview", "Eye"),
            (ViewMode::Code, "Code", "Code"),
        ] {
            let selected = self.mode == mode;
            if selected {
                ui.painter().rect_filled(
                    egui::Rect::from_min_size(ui.next_widget_position(), egui::vec2(32., 26.)),
                    egui::CornerRadius::from(attachment_content::points(
                        presentation.radii["control"],
                    )),
                    attachment_content::color(presentation, "background")?,
                );
            }
            let title = i18n.text(title)?;
            let response = attachment_content::ghost_labeled_tinted(
                ui,
                presentation,
                (glyph, None),
                egui::vec2(32., 26.),
                attachment_content::color(presentation, if selected { "text" } else { "muted" })?,
            )?
            .on_hover_text(&title);
            response.widget_info(|| {
                egui::WidgetInfo::selected(
                    egui::WidgetType::SelectableLabel,
                    true,
                    selected,
                    &title,
                )
            });
            if response.clicked() && !selected {
                self.mode = mode;
                transition = Some(mode);
            }
        }
        Ok(transition)
    }
}
