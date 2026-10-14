//! Attachment classification and content operands.
use super::AttachmentOverlay;
use crate::{
    Attachment, I18n,
    browser::attachment_content::{
        AttachmentContent, AttachmentSurface, AttachmentUiError, FileType, ImageFit, TextView,
        color, points,
    },
};
use maestro_app::browser::utils::attachment_utils::AttachmentType;
use maestro_theme::BrandPresentation;
use std::rc::Rc;
/// Select the first matching document presentation.
pub(super) fn file_type(a: &Attachment) -> FileType {
    if a.r#type == AttachmentType::Image {
        FileType::Image
    } else if a.mime_type == "application/pdf" {
        FileType::Pdf
    } else if a.mime_type.contains("wordprocessingml") {
        FileType::Docx
    } else if a.mime_type.contains("presentationml")
        || a.file_name.to_ascii_lowercase().ends_with(".pptx")
    {
        FileType::Pptx
    } else if a.mime_type.contains("spreadsheetml")
        || a.mime_type.contains("ms-excel")
        || [".xlsx", ".xls"]
            .iter()
            .any(|s| a.file_name.to_ascii_lowercase().ends_with(s))
    {
        FileType::Excel
    } else {
        FileType::Text
    }
}
impl AttachmentOverlay {
    /// Copy only the strings needed after releasing the record borrow.
    pub(super) fn body<F: ?Sized>(
        &mut self,
        ui: &mut egui::Ui,
        i: &I18n<F>,
        p: &BrandPresentation<'_>,
    ) -> Result<egui::Rect, AttachmentUiError> {
        if let Some(rect) = self.error_panel(ui, i, p)? {
            return Ok(rect);
        }
        let (kind, name, source, text, reload) = self.content_data()?;
        self.changed = false;
        let text = if kind != FileType::Image
            && (self.extracted || matches!(kind, FileType::Text | FileType::Pptx))
        {
            match text {
                Some(text) => text,
                None => i.text(if kind == FileType::Text && !self.extracted {
                    "No content available"
                } else {
                    "No text content available"
                })?,
            }
        } else {
            String::new()
        };
        let content = if kind == FileType::Image {
            AttachmentContent::Image {
                source: &source,
                alt: &name,
                fit: ImageFit::Contain,
            }
        } else if self.extracted || matches!(kind, FileType::Text | FileType::Pptx) {
            AttachmentContent::Text {
                text: &text,
                view: if self.extracted {
                    TextView::Extracted
                } else if kind == FileType::Pptx {
                    TextView::Presentation
                } else {
                    TextView::Plain
                },
            }
        } else {
            AttachmentContent::Preview {
                file_type: kind,
                attachment: Rc::clone(&self.attachment),
                reload,
            }
        };
        self.present(ui, content, p, kind)
    }
    /// Capture live content operands without cloning the full record.
    fn content_data(
        &mut self,
    ) -> Result<(FileType, String, String, Option<String>, bool), AttachmentUiError> {
        Ok({
            let a = self
                .attachment
                .try_borrow()
                .map_err(|_| AttachmentUiError::BorrowConflict)?;
            let kind = file_type(&a);
            let changed = self
                .previous
                .as_ref()
                .is_none_or(|(previous, mime, name, content)| {
                    *previous != kind
                        || mime != &a.mime_type
                        || name != &a.file_name
                        || content != &a.content
                });
            if changed {
                self.previous = Some((
                    kind,
                    a.mime_type.clone(),
                    a.file_name.clone(),
                    a.content.clone(),
                ));
            }
            let text = if kind != FileType::Image
                && (self.extracted || matches!(kind, FileType::Text | FileType::Pptx))
            {
                a.extracted_text.as_ref().filter(|s| !s.is_empty()).cloned()
            } else {
                None
            };
            let source = if kind == FileType::Image {
                format!("data:{};base64,{}", a.mime_type, a.content)
            } else {
                String::new()
            };
            (
                kind,
                a.file_name.clone(),
                source,
                text,
                changed || self.changed,
            )
        })
    }
    /// Render a document-display failure without removing header controls.
    fn error_panel<F: ?Sized>(
        &self,
        ui: &mut egui::Ui,
        i: &I18n<F>,
        p: &BrandPresentation<'_>,
    ) -> Result<Option<egui::Rect>, AttachmentUiError> {
        let Some(error) = self.error.as_ref().filter(|s| !s.is_empty()) else {
            return Ok(None);
        };
        self.host.remove(self.id);
        let margin = points(p.spacing["md"]);
        let spacing = points(p.spacing["xs"]);
        let font = ui.style().text_styles[&egui::TextStyle::Body].clone();
        let tint = color(p, "error")?;
        let available = ui.available_rect_before_wrap();
        let wrap = (available.width().min(672.) - 2. * margin).max(0.);
        let heading = ui
            .painter()
            .layout(i.text("Error loading file")?, font.clone(), tint, wrap);
        let detail = ui
            .painter()
            .layout(error.clone(), font, tint.gamma_multiply(0.9), wrap);
        let size = egui::vec2(
            heading.size().x.max(detail.size().x) + 2. * margin,
            heading.size().y + detail.size().y + spacing + 2. * margin,
        );
        let rect = egui::Rect::from_center_size(available.center(), size);
        ui.painter().rect(
            rect,
            egui::CornerRadius::from(points(p.radii["panel"])),
            color(p, "panel")?,
            egui::Stroke::new(1., tint),
            egui::StrokeKind::Inside,
        );
        let origin = rect.min + egui::vec2(margin, margin);
        let detail_origin = origin + egui::vec2(0., heading.size().y + spacing);
        ui.painter().galley(origin, heading, tint);
        ui.painter()
            .galley(detail_origin, detail, tint.gamma_multiply(0.9));
        Ok(Some(rect))
    }
    /// Place selected content in its bounded viewport container.
    fn present(
        &mut self,
        ui: &egui::Ui,
        content: AttachmentContent<'_>,
        p: &BrandPresentation<'_>,
        kind: FileType,
    ) -> Result<egui::Rect, AttachmentUiError> {
        let width = if self.extracted || kind == FileType::Text {
            896.
        } else if kind == FileType::Excel || kind == FileType::Image {
            ui.available_width()
        } else {
            1000.
        };
        let mut rect = ui.available_rect_before_wrap();
        rect.set_width(rect.width().min(width));
        rect = rect.translate(egui::vec2((ui.available_width() - rect.width()) / 2., 0.));
        match self.host.present(
            AttachmentSurface {
                id: self.id,
                rect,
                clip_rect: ui.clip_rect(),
                pixels_per_point: ui.ctx().pixels_per_point(),
                opacity: 1.,
                occluders: Vec::new(),
                content,
            },
            p,
        ) {
            Err(AttachmentUiError::Presentation(message)) => {
                self.error = Some(message);
                ui.ctx().request_repaint();
                Ok(rect)
            }
            other => other.map(|()| rect),
        }
    }
}
