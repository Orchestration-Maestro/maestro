//! Retained selection between a document preview and its original source.
/// Native control group and its page element.
#[cfg(target_arch = "wasm32")]
pub mod dom;

use crate::components::copy_button::LocalizedControlError;
use crate::{
    I18n,
    browser::attachment_content::{self},
};
use std::{cell::Cell, rc::Rc};
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
    /// Current selected presentation, shared with a mounted native group.
    selection: Rc<Cell<ViewMode>>,
    /// Per-instance toolkit identity, allocated by the first retained render.
    id: Option<egui::Id>,
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
            selection: Rc::new(Cell::new(ViewMode::Preview)),
            id: None,
        }
    }
    /// Retain this selection without sharing a retained drawing identity.
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn shared(&self) -> Self {
        Self {
            selection: Rc::clone(&self.selection),
            id: None,
        }
    }
    /// The current selected presentation.
    #[must_use]
    pub fn mode(&self) -> ViewMode {
        self.selection.get()
    }
    /// Select a presentation without publishing it; a later render or update shows it.
    pub fn set_mode(&self, mode: ViewMode) {
        self.selection.set(mode);
    }
    /// Draw both controls, returning only a changed user selection.
    /// # Errors
    /// Returns localization, glyph or brand projection failures.
    pub fn render<F: ?Sized>(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &I18n<F>,
        presentation: &maestro_theme::BrandPresentation<'_>,
    ) -> Result<Option<ViewMode>, LocalizedControlError> {
        use attachment_content::{color, points};
        attachment_content::style(ui, presentation).map_err(LocalizedControlError::Presentation)?;
        let radius = egui::CornerRadius::from(points(presentation.radii["control"]));
        let id = *self.id.get_or_insert_with(attachment_content::identity);
        ui.push_id(id, |ui| {
            egui::Frame::new()
                .fill(
                    color(presentation, "panel")
                        .map_err(LocalizedControlError::Presentation)?
                        .gamma_multiply(0.6),
                )
                .stroke(egui::Stroke::new(
                    1.,
                    color(presentation, "border").map_err(LocalizedControlError::Presentation)?,
                ))
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
        &self,
        ui: &mut egui::Ui,
        i18n: &I18n<F>,
        presentation: &maestro_theme::BrandPresentation<'_>,
    ) -> Result<Option<ViewMode>, LocalizedControlError> {
        let mut transition = None;
        for (mode, title, glyph) in [
            (ViewMode::Preview, "Preview", "Eye"),
            (ViewMode::Code, "Code", "Code"),
        ] {
            let selected = self.mode() == mode;
            if selected {
                ui.painter().rect_filled(
                    egui::Rect::from_min_size(ui.next_widget_position(), egui::vec2(32., 26.)),
                    egui::CornerRadius::from(attachment_content::points(
                        presentation.radii["control"],
                    )),
                    attachment_content::color(presentation, "background")
                        .map_err(LocalizedControlError::Presentation)?,
                );
            }
            let title = i18n.text(title)?;
            let font = egui::TextStyle::Button.resolve(ui.style());
            let response = attachment_content::ghost_labeled_tinted(
                ui,
                presentation,
                (glyph, None),
                (egui::vec2(32., 26.), 0., font),
                (
                    attachment_content::color(
                        presentation,
                        if selected { "text" } else { "muted" },
                    )
                    .map_err(LocalizedControlError::Presentation)?,
                    attachment_content::GhostHover::Text,
                ),
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
                self.set_mode(mode);
                transition = Some(mode);
            }
        }
        Ok(transition)
    }
}
