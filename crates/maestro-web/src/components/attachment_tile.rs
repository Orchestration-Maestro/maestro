//! Attachment thumbnails with optional removal.
use crate::{
    Attachment, AttachmentOverlay, I18n,
    browser::attachment_content::{
        AttachmentContent, AttachmentHost, AttachmentSurface, AttachmentUiError, ImageFit, color,
        glyph, identity, points, style,
    },
};
use maestro_app::{browser::utils::attachment_utils::AttachmentType, truncate_utf16};
use maestro_theme::BrandPresentation;
use std::{cell::RefCell, rc::Rc};
/// A caller-owned attachment tile.
pub struct AttachmentTile {
    /// Original mutable record.
    pub attachment: Rc<RefCell<Attachment>>,
    /// Whether to allocate a remove control.
    pub show_delete: bool,
    /// Optional removal notification.
    pub on_delete: Option<Rc<dyn Fn()>>,
    /// Platform content owner.
    host: Rc<dyn AttachmentHost>,
    /// Independent surface identity.
    id: egui::Id,
}
impl AttachmentTile {
    /// Construct a tile with deletion disabled.
    #[must_use]
    pub fn new(attachment: Rc<RefCell<Attachment>>, host: Rc<dyn AttachmentHost>) -> Self {
        Self {
            attachment,
            host,
            show_delete: false,
            on_delete: None,
            id: identity(),
        }
    }
    /// Draw a tile, returning an independently retained overlay when opened.
    /// # Errors
    /// Propagates localization, glyph, platform and record-borrow failures.
    pub fn render<F: ?Sized>(
        &mut self,
        ui: &mut egui::Ui,
        i: &I18n<F>,
        p: &BrandPresentation<'_>,
    ) -> Result<Option<AttachmentOverlay>, AttachmentUiError> {
        ui.scope(|ui| self.draw(ui, i, p)).inner
    }
    /// Draw without retaining a record borrow across replaceable effects.
    fn draw<F: ?Sized>(
        &self,
        ui: &mut egui::Ui,
        i: &I18n<F>,
        p: &BrandPresentation<'_>,
    ) -> Result<Option<AttachmentOverlay>, AttachmentUiError> {
        style(ui, p)?;
        let (name, preview, pdf, excel) = self.thumbnail_data()?;
        let (_, rect) = ui.allocate_space(egui::vec2(64., 64.));
        let badge = (pdf && preview.is_some()).then_some(rect.with_min_y(rect.bottom() - 19.));
        let image = badge.map_or(rect, |badge| rect.with_max_y(badge.top()));
        let response = ui
            .interact(image, self.id, egui::Sense::click())
            .on_hover_text(&name)
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        let hovered = response.hovered()
            && ui.input(|i| i.pointer.hover_pos().is_some_and(|pos| image.contains(pos)));
        let opacity = if hovered { 0.8 } else { 1. };
        let muted = color(p, "muted")?.gamma_multiply(opacity);
        ui.painter().rect(
            rect,
            egui::CornerRadius::from(points(p.radii["panel"])),
            color(p, "panel")?.gamma_multiply(opacity),
            egui::Stroke::new(1., color(p, "border")?),
            egui::StrokeKind::Inside,
        );
        let (removed, remove_rect) = self.delete(ui, rect, i, p)?;
        let mut occluders: Vec<_> = remove_rect.into_iter().collect();
        if let Some(source) = preview {
            if let Some(badge) = badge {
                occluders.push(badge);
                pdf_badge(ui, badge, i, p)?;
            }
            self.host.present(
                AttachmentSurface {
                    id: self.id,
                    rect,
                    clip_rect: ui.clip_rect(),
                    pixels_per_point: ui.ctx().pixels_per_point(),
                    opacity,
                    occluders,
                    content: AttachmentContent::Image {
                        source: &source,
                        alt: &name,
                        fit: ImageFit::Cover,
                    },
                },
                p,
            )?;
        } else {
            self.host.remove(self.id);
            fallback(ui, rect, &name, excel, muted)?;
        }
        Ok((response.clicked() && !removed).then(|| {
            AttachmentOverlay::open(Rc::clone(&self.attachment), None, Rc::clone(&self.host))
        }))
    }
    /// Copy only operands needed outside the record borrow.
    fn thumbnail_data(&self) -> Result<(String, Option<String>, bool, bool), AttachmentUiError> {
        let a = self
            .attachment
            .try_borrow()
            .map_err(|_| AttachmentUiError::BorrowConflict)?;
        let mime = if a.r#type == AttachmentType::Image {
            a.mime_type.as_str()
        } else {
            "image/png"
        };
        Ok((
            a.file_name.clone(),
            a.preview
                .as_ref()
                .filter(|s| !s.is_empty())
                .map(|s| format!("data:{mime};base64,{s}")),
            a.mime_type == "application/pdf",
            a.mime_type.contains("spreadsheetml")
                || [".xlsx", ".xls"]
                    .iter()
                    .any(|s| a.file_name.to_ascii_lowercase().ends_with(s)),
        ))
    }
    /// Keep the remove target allocated while its paint is hidden.
    fn delete<F: ?Sized>(
        &self,
        ui: &egui::Ui,
        tile_rect: egui::Rect,
        i: &I18n<F>,
        p: &BrandPresentation<'_>,
    ) -> Result<(bool, Option<egui::Rect>), AttachmentUiError> {
        if !self.show_delete {
            return Ok((false, None));
        }
        let rect = egui::Rect::from_min_size(
            tile_rect.right_top() + egui::vec2(-16., -4.),
            egui::vec2(20., 20.),
        );
        let button = ui
            .interact(rect, self.id.with("remove"), egui::Sense::click())
            .on_hover_text(i.text("Remove")?);
        let hovered = ui.input(|i| {
            i.pointer
                .hover_pos()
                .is_some_and(|pos| tile_rect.contains(pos))
        });
        let visible = !self.host.hover_capable()? || hovered || button.hovered();
        if visible {
            ui.painter().circle(
                rect.center(),
                10.,
                color(p, "panel")?,
                egui::Stroke::new(1., color(p, "border")?),
            );
            let role = if button.hovered() { "text" } else { "muted" };
            glyph(ui, rect.shrink(4.), "X", color(p, role)?)?;
        }
        if button.clicked()
            && let Some(callback) = &self.on_delete
        {
            callback();
        }
        Ok((button.clicked(), visible.then_some(rect)))
    }
}
impl Drop for AttachmentTile {
    fn drop(&mut self) {
        self.host.remove(self.id);
    }
}
/// Scale caption text from the caller-selected body face.
fn caption(ui: &egui::Ui) -> egui::FontId {
    let mut font = ui.style().text_styles[&egui::TextStyle::Body].clone();
    font.size *= 0.625;
    font
}
/// Paint the document glyph and whole-character filename prefix.
fn fallback(
    ui: &egui::Ui,
    rect: egui::Rect,
    name: &str,
    excel: bool,
    tint: egui::Color32,
) -> Result<(), AttachmentUiError> {
    glyph(
        ui,
        egui::Rect::from_center_size(rect.center() - egui::vec2(0., 8.), egui::vec2(20., 20.)),
        if excel { "FileSpreadsheet" } else { "FileText" },
        tint,
    )?;
    let label = if name.encode_utf16().count() > 10 {
        format!("{}...", truncate_utf16(name, 8))
    } else {
        name.to_owned()
    };
    let bounds = rect.shrink(8.);
    let mut job = egui::text::LayoutJob::simple(label, caption(ui), tint, bounds.width());
    job.wrap.max_rows = 1;
    let painter = ui
        .painter()
        .with_clip_rect(bounds.intersect(ui.clip_rect()));
    let galley = painter.layout_job(job);
    let pos = bounds.center_bottom() - egui::vec2(galley.size().x / 2., galley.size().y);
    painter.galley(pos, galley, tint);
    Ok(())
}

/// Paint the badge above the image with rounded lower corners.
fn pdf_badge<F: ?Sized>(
    ui: &egui::Ui,
    badge: egui::Rect,
    i: &I18n<F>,
    p: &BrandPresentation<'_>,
) -> Result<(), AttachmentUiError> {
    ui.painter().rect_filled(
        badge,
        egui::CornerRadius {
            nw: 0,
            ne: 0,
            ..egui::CornerRadius::from(points(p.radii["panel"]))
        },
        color(p, "background")?,
    );
    ui.painter().text(
        badge.center(),
        egui::Align2::CENTER_CENTER,
        i.text("PDF")?,
        caption(ui),
        color(p, "muted")?,
    );
    Ok(())
}
