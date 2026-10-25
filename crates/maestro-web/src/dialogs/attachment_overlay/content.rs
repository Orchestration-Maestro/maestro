//! Attachment classification and content operands.
use super::LegacyOverlay;
use crate::dialogs::attachment_overlay::AttachmentUiError;
use crate::{
    Attachment, I18n,
    browser::attachment_content::{
        AttachmentContent, AttachmentSurface, FileType, ImageFit, TextView,
    },
};
use maestro_app::browser::utils::attachment_utils::AttachmentType;
use maestro_theme::BrandPresentation;
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
impl LegacyOverlay {
    /// Copy only the strings needed after releasing the record borrow.
    pub(super) fn body<F: ?Sized>(
        &mut self,
        ui: &mut egui::Ui,
        i: &I18n<F>,
        p: &BrandPresentation<'_>,
    ) -> Result<egui::Rect, AttachmentUiError> {
        if let Some(error) = self.error.as_ref().filter(|s| !s.is_empty()) {
            return Ok(self.error_view.present(
                self.host.as_ref(),
                ui,
                (self.id, Some(self.id), false),
                ([i.text("Error loading file")?, error.clone()], p),
            )?);
        }
        let (kind, name, source, text, reload) = self.content_data()?;
        self.changed = false;
        if reload || kind != FileType::Pdf || self.extracted {
            self.pdf.reset(false);
        }
        if reload || kind != FileType::Excel || self.extracted {
            self.excel.reset(false);
        }
        match (kind, self.extracted) {
            (FileType::Excel, false) => return self.excel_body(ui, i, p, reload),
            (FileType::Pdf, false) => return self.pdf_body(ui, i, p, reload),
            _ => {}
        }
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
                attachment: std::sync::Arc::clone(&self.attachment),
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
                .try_read()
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
    /// Advance the shared owned document through the raw attachment byte domain.
    fn pdf_body<F: ?Sized>(
        &mut self,
        ui: &egui::Ui,
        i: &I18n<F>,
        p: &BrandPresentation<'_>,
        reload: bool,
    ) -> Result<egui::Rect, AttachmentUiError> {
        let result = if reload {
            let bytes = {
                let attachment = self
                    .attachment
                    .try_read()
                    .map_err(|_| AttachmentUiError::BorrowConflict)?;
                crate::utils::base64::decode(&attachment.content).map_err(|e| e.to_string())
            };
            bytes.and_then(|bytes| self.pdf.open(bytes))
        } else {
            Ok(())
        };
        if let Err(message) = result.and_then(|()| self.pdf.advance(ui.ctx())) {
            self.error = Some(super::pdf::detail(&message, i)?);
            ui.ctx().request_repaint();
            return Ok(ui.available_rect_before_wrap());
        }
        let mut rect = ui.available_rect_before_wrap();
        rect.set_width(rect.width().min(1000.));
        rect = rect.translate(egui::vec2((ui.available_width() - rect.width()) / 2., 0.));
        match self.pdf.present(
            self.host.as_ref(),
            ui,
            (self.id, rect, Some(self.id), false),
            p,
        ) {
            Err(crate::browser::attachment_content::AttachmentHostError::Presentation(message)) => {
                if !message.is_empty() {
                    self.pdf.reset(false);
                }
                self.error = Some(message);
                ui.ctx().request_repaint();
                Ok(rect)
            }
            other => other.map(|()| rect).map_err(AttachmentUiError::Host),
        }
    }
    /// Prepare raw attachment bytes once and retain all native sheet bodies.
    fn excel_body<F: ?Sized>(
        &mut self,
        ui: &mut egui::Ui,
        i: &I18n<F>,
        p: &BrandPresentation<'_>,
        reload: bool,
    ) -> Result<egui::Rect, AttachmentUiError> {
        if reload {
            let bytes = {
                let attachment = self
                    .attachment
                    .try_read()
                    .map_err(|_| AttachmentUiError::BorrowConflict)?;
                crate::utils::base64::decode(&attachment.content).map_err(|e| e.to_string())
            };
            let result = bytes.and_then(|bytes| self.excel.open(&bytes));
            if let Err(message) = result {
                self.error = Some(super::error::detail(
                    &message,
                    i,
                    "Failed to load spreadsheet",
                )?);
                ui.ctx().request_repaint();
                return Ok(ui.available_rect_before_wrap());
            }
        }
        let rect = ui.available_rect_before_wrap();
        match self.excel.present(
            self.host.as_ref(),
            ui,
            (self.id, rect, Some(self.id), false),
            (p, &i.text("Error loading spreadsheet")?),
        ) {
            Err(crate::browser::attachment_content::AttachmentHostError::Presentation(message)) => {
                if !message.is_empty() {
                    self.excel.reset(false);
                }
                self.error = Some(message);
                ui.ctx().request_repaint();
                Ok(rect)
            }
            other => other.map(|()| rect).map_err(AttachmentUiError::Host),
        }
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
            Err(crate::browser::attachment_content::AttachmentHostError::Presentation(message)) => {
                self.error = Some(message);
                ui.ctx().request_repaint();
                Ok(rect)
            }
            other => other.map(|()| rect).map_err(AttachmentUiError::Host),
        }
    }
}
