//! Retained full-screen attachment preview.
mod ui_error;
use crate::{
    I18n,
    browser::attachment_content::{
        AttachmentHost, FileType, color, ghost, identity, points, style,
    },
};
use maestro_theme::BrandPresentation;
use std::rc::Rc;
pub use ui_error::AttachmentUiError;
mod content;
#[cfg(target_arch = "wasm32")]
mod dom;
pub(crate) mod error;
pub(crate) mod excel;
pub(crate) mod pdf;
/// A separately owned attachment overlay.
struct LegacyOverlay {
    /// Live original record.
    attachment: crate::SharedAttachment,
    /// Platform effects.
    host: Rc<dyn AttachmentHost>,
    /// Per-instance surface identity.
    id: egui::Id,
    /// Optional notification, taken before invocation.
    on_close: Option<Rc<dyn Fn()>>,
    /// Whether extracted text is selected.
    extracted: bool,
    /// Last file-display failure.
    error: Option<String>,
    /// Owned progressive document and native pages.
    pdf: pdf::Pdf,
    /// Owned spreadsheet tables and selected sheet.
    excel: excel::Excel,
    /// Native error panel, independent of the failure latch.
    error_view: error::Error,
    /// Last rendered file operands.
    previous: Option<(FileType, String, String, String)>,
    /// A selected-mode change needs dispatch.
    changed: bool,
    /// Closed owners render nothing.
    closed: bool,
}
impl LegacyOverlay {
    /// Retain one independent overlay and its original record.
    #[must_use]
    pub fn open(
        attachment: crate::SharedAttachment,
        on_close: Option<Rc<dyn Fn()>>,
        host: Rc<dyn AttachmentHost>,
    ) -> Self {
        Self {
            attachment,
            host,
            id: identity(),
            on_close,
            extracted: false,
            error: None,
            pdf: pdf::Pdf::default(),
            excel: excel::Excel::default(),
            error_view: error::Error::default(),
            previous: None,
            changed: true,
            closed: false,
        }
    }
    /// Draw/process this overlay; false means the caller may release it.
    /// # Errors
    /// Propagates localization, glyph, borrow and platform failures.
    pub fn render<F: ?Sized>(
        &mut self,
        context: &egui::Context,
        i18n: &I18n<F>,
        p: &BrandPresentation<'_>,
    ) -> Result<bool, AttachmentUiError> {
        if self.closed {
            return Ok(false);
        }
        if context.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.close()?;
            return Ok(false);
        }
        self.host.set_overlay(self.id, true)?;
        let mut close = false;
        let area = egui::Area::new(self.id)
            .order(egui::Order::Foreground)
            .fade_in(false)
            .fixed_pos(context.content_rect().min)
            .show(context, |ui| -> Result<(), AttachmentUiError> {
                style(ui, p).map_err(|error| AttachmentUiError::Presentation(error.to_string()))?;
                ui.set_min_size(context.content_rect().size());
                ui.set_max_size(context.content_rect().size());
                ui.painter().rect_filled(
                    context.content_rect(),
                    0.,
                    color(p, "background")
                        .map_err(|error| AttachmentUiError::Presentation(error.to_string()))?
                        .gamma_multiply(0.9),
                );
                let header_background = egui::Rect::from_min_size(
                    context.content_rect().min,
                    egui::vec2(context.content_rect().width(), 48.),
                );
                ui.painter().rect_filled(
                    header_background,
                    0.,
                    color(p, "background")
                        .map_err(|error| AttachmentUiError::Presentation(error.to_string()))?
                        .gamma_multiply(0.95),
                );
                ui.painter().hline(
                    header_background.x_range(),
                    header_background.bottom(),
                    egui::Stroke::new(
                        1.,
                        color(p, "border")
                            .map_err(|error| AttachmentUiError::Presentation(error.to_string()))?,
                    ),
                );
                ui.horizontal(|ui| self.header(ui, i18n, p, &mut close))
                    .inner?;
                let bounds = egui::Rect::from_min_max(
                    egui::pos2(context.content_rect().left(), header_background.bottom()),
                    context.content_rect().max,
                );
                let mut body_ui = ui.new_child(egui::UiBuilder::new().max_rect(bounds));
                self.body(&mut body_ui, i18n, p)?;
                Ok(())
            });
        area.inner?;
        if close {
            self.close()?;
        }
        Ok(!self.closed)
    }
    /// Revoke retained state before notifying, then remove owned surfaces.
    fn close(&mut self) -> Result<(), AttachmentUiError> {
        self.closed = true;
        self.extracted = false;
        self.error = None;
        self.previous = None;
        self.pdf.reset(false);
        self.excel.reset(false);
        if let Some(callback) = self.on_close.take() {
            callback();
        }
        self.host.remove(self.id);
        self.host
            .set_overlay(self.id, false)
            .map_err(AttachmentUiError::Host)
    }
    /// Draw the filename, conditional toggle and ghost controls.
    fn header<F: ?Sized>(
        &mut self,
        ui: &mut egui::Ui,
        i: &I18n<F>,
        p: &BrandPresentation<'_>,
        close: &mut bool,
    ) -> Result<(), AttachmentUiError> {
        let name = self
            .attachment
            .try_read()
            .map_err(|_| AttachmentUiError::BorrowConflict)?
            .file_name
            .clone();
        ui.set_min_height(48.);
        let header_rect = ui
            .max_rect()
            .shrink2(egui::vec2(points(p.spacing["md"]), points(p.spacing["xs"])));
        let ui = &mut ui.new_child(
            egui::UiBuilder::new()
                .max_rect(header_rect)
                .layout(egui::Layout::right_to_left(egui::Align::Center)),
        );
        let (_, close_rect) = ui.allocate_space(egui::vec2(32., 32.));
        let (_, download_rect) = ui.allocate_space(egui::vec2(32., 32.));
        self.toggle(ui, i)?;
        let mut download_ui = ui.new_child(egui::UiBuilder::new().max_rect(download_rect));
        if ghost(&mut download_ui, p, "Download")?.clicked() {
            self.host
                .download(std::sync::Arc::clone(&self.attachment))?;
        }
        let mut close_ui = ui.new_child(egui::UiBuilder::new().max_rect(close_rect));
        *close |= ghost(&mut close_ui, p, "X")?.clicked();
        let mut name_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(ui.available_rect_before_wrap())
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        name_ui.add(
            egui::Label::new(
                egui::RichText::new(name).size(points(p.typography["body"].size) * 0.875),
            )
            .truncate(),
        );
        Ok(())
    }
    /// Select a supported document mode without dispatching repeated selections.
    fn toggle<F: ?Sized>(
        &mut self,
        ui: &mut egui::Ui,
        i: &I18n<F>,
    ) -> Result<(), AttachmentUiError> {
        let (kind, enabled) = {
            let a = self
                .attachment
                .try_read()
                .map_err(|_| AttachmentUiError::BorrowConflict)?;
            (
                content::file_type(&a),
                a.extracted_text.as_ref().is_some_and(|s| !s.is_empty()),
            )
        };
        if !enabled {
            return Ok(());
        }
        let label = match kind {
            FileType::Pdf => "PDF",
            FileType::Docx => "Document",
            FileType::Excel => "Spreadsheet",
            _ => return Ok(()),
        };
        let labels = [i.text(label)?, i.text("Text")?];
        let font = egui::TextStyle::Button.resolve(ui.style());
        let width = labels
            .iter()
            .map(|label| {
                let galley =
                    ui.painter()
                        .layout_no_wrap(label.clone(), font.clone(), egui::Color32::WHITE);
                galley.size().x
            })
            .sum::<f32>()
            + 4. * ui.spacing().button_padding.x
            + ui.spacing().item_spacing.x;
        let (_, rect) = ui.allocate_space(egui::vec2(width, 32.));
        let mut toggle_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(rect)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        for (index, label) in [false, true].into_iter().zip(labels) {
            if toggle_ui
                .selectable_label(self.extracted == index, label)
                .clicked()
                && self.extracted != index
            {
                self.extracted = index;
                self.changed = true;
                self.error = None;
            }
        }
        Ok(())
    }
}
impl Drop for LegacyOverlay {
    fn drop(&mut self) {
        if !self.closed {
            self.host.remove(self.id);
            let _ = self
                .host
                .set_overlay(self.id, false)
                .map_err(AttachmentUiError::Host);
        }
    }
}

/// A separately owned attachment overlay.
pub struct AttachmentOverlay {
    /// Rendering owner selected at construction.
    state: Overlay,
}
/// Retained presentation selected by the caller.
enum Overlay {
    /// Canvas presentation.
    Legacy(Box<LegacyOverlay>),
    /// Native document presentation.
    #[cfg(target_arch = "wasm32")]
    Native(Rc<dom::State>),
}
impl AttachmentOverlay {
    /// Retain one independent overlay and its original record.
    #[must_use]
    pub fn open(
        attachment: crate::SharedAttachment,
        on_close: Option<Rc<dyn Fn()>>,
        host: Rc<dyn AttachmentHost>,
    ) -> Self {
        Self {
            state: Overlay::Legacy(Box::new(LegacyOverlay::open(attachment, on_close, host))),
        }
    }
    /// Draw/process this overlay; false means the caller may release it.
    /// # Errors
    /// Propagates localization, glyph, borrow and platform failures.
    pub fn render<F: ?Sized>(
        &mut self,
        context: &egui::Context,
        i18n: &I18n<F>,
        p: &BrandPresentation<'_>,
    ) -> Result<bool, AttachmentUiError> {
        match &mut self.state {
            Overlay::Legacy(overlay) => overlay.render(context, i18n, p),
            #[cfg(target_arch = "wasm32")]
            Overlay::Native(_) => Err(AttachmentUiError::Presentation(
                "Native overlay has no canvas rendering".into(),
            )),
        }
    }
}
