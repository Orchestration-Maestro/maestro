//! Retained full-screen attachment preview.
use crate::{
    Attachment, I18n,
    browser::attachment_content::{
        AttachmentHost, AttachmentUiError, FileType, color, ghost, identity, points, style,
    },
};
use maestro_theme::BrandPresentation;
use std::{cell::RefCell, rc::Rc};
mod content;
/// A separately owned attachment overlay.
pub struct AttachmentOverlay {
    /// Live original record.
    attachment: Rc<RefCell<Attachment>>,
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
    /// Last rendered file operands.
    previous: Option<(FileType, String, String, String)>,
    /// A selected-mode change needs dispatch.
    changed: bool,
    /// Closed owners render nothing.
    closed: bool,
}
impl AttachmentOverlay {
    /// Retain one independent overlay and its original record.
    #[must_use]
    pub fn open(
        attachment: Rc<RefCell<Attachment>>,
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
                style(ui, p)?;
                ui.set_min_size(context.content_rect().size());
                ui.set_max_size(context.content_rect().size());
                ui.painter().rect_filled(
                    context.content_rect(),
                    0.,
                    color(p, "background")?.gamma_multiply(0.9),
                );
                let header_background = egui::Rect::from_min_size(
                    context.content_rect().min,
                    egui::vec2(context.content_rect().width(), 48.),
                );
                ui.painter().rect_filled(
                    header_background,
                    0.,
                    color(p, "background")?.gamma_multiply(0.95),
                );
                ui.painter().hline(
                    header_background.x_range(),
                    header_background.bottom(),
                    egui::Stroke::new(1., color(p, "border")?),
                );
                let header = ui
                    .horizontal(|ui| self.header(ui, i18n, p, &mut close))
                    .inner?;
                let bounds = egui::Rect::from_min_max(
                    egui::pos2(context.content_rect().left(), header.bottom()),
                    context.content_rect().max,
                );
                let mut body_ui = ui.new_child(egui::UiBuilder::new().max_rect(bounds));
                let body = self.body(&mut body_ui, i18n, p)?;
                if context.input(|i| i.pointer.primary_clicked())
                    && let Some(pos) = context.input(|i| i.pointer.interact_pos())
                    && !header.contains(pos)
                    && !body.contains(pos)
                {
                    close = true;
                }
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
        if let Some(callback) = self.on_close.take() {
            callback();
        }
        self.host.remove(self.id);
        self.host.set_overlay(self.id, false)
    }
    /// Draw the filename, conditional toggle and ghost controls.
    fn header<F: ?Sized>(
        &mut self,
        ui: &mut egui::Ui,
        i: &I18n<F>,
        p: &BrandPresentation<'_>,
        close: &mut bool,
    ) -> Result<egui::Rect, AttachmentUiError> {
        let name = self
            .attachment
            .try_borrow()
            .map_err(|_| AttachmentUiError::BorrowConflict)?
            .file_name
            .clone();
        let start = ui.cursor().min;
        ui.set_min_height(48.);
        ui.with_layout(
            egui::Layout::right_to_left(egui::Align::Center),
            |ui| -> Result<(), AttachmentUiError> {
                let c = ghost(ui, p, "X")?;
                *close |= c.clicked();
                let d = ghost(ui, p, "Download")?;
                if d.clicked() {
                    self.host.download(Rc::clone(&self.attachment))?;
                }
                self.toggle(ui, i)?;
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(name).size(points(p.typography["body"].size) * 0.875),
                    )
                    .truncate(),
                );
                Ok(())
            },
        )
        .inner?;
        Ok(egui::Rect::from_min_max(
            start,
            egui::pos2(ui.max_rect().right(), ui.min_rect().bottom()),
        ))
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
                .try_borrow()
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
        for (index, label) in [(true, "Text"), (false, label)] {
            if ui
                .selectable_label(self.extracted == index, i.text(label)?)
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
impl Drop for AttachmentOverlay {
    fn drop(&mut self) {
        if !self.closed {
            self.host.remove(self.id);
            let _ = self.host.set_overlay(self.id, false);
        }
    }
}
