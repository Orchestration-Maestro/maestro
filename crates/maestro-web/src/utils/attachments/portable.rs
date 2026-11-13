//! Native PDF page text and opaque rasterization.
use super::AttachmentError;
use base64::{Engine, engine::general_purpose::STANDARD};
use hayro::hayro_syntax::{
    Pdf,
    object::{Array, Dict, Rect as PdfRect},
    page::{Page, Rotation},
};
use hayro::kurbo::Affine;
use hayro::vello_cpu::color::palette::css::WHITE;
use hayro::vello_cpu::{Pixmap, RasterizerSettings, RenderContext, Resources, TargetInit};
use hayro::{RenderCache, RenderSettings};
use hayro_interpret::{InterpreterCache, InterpreterSettings, TransformExt};
use num_traits::ToPrimitive;
use std::fmt::Write;

/// An owned native document used for ordered page rendering.
pub struct PdfDocument {
    /// Native bytes, cross references and page tree.
    pdf: Pdf,
}
/// An opaque rendered page at its raster dimensions.
pub struct PdfPage {
    /// Raster width in pixels.
    pub width: u16,
    /// Raster height in pixels.
    pub height: u16,
    /// Complete encoded PNG bytes.
    pub png: Vec<u8>,
}
impl PdfDocument {
    /// Open supplied original bytes.
    /// # Errors
    /// Returns a PDF failure when the native document cannot be opened.
    pub fn new(bytes: Vec<u8>) -> Result<Self, AttachmentError> {
        Ok(Self {
            pdf: Pdf::new(bytes).map_err(|error| AttachmentError::Pdf(format!("{error:?}")))?,
        })
    }
    /// Return the number of pages in document order.
    #[must_use]
    pub fn num_pages(&self) -> usize {
        self.pdf.pages().len()
    }
    /// Render a one-based page at scale 1.5 on opaque white.
    /// # Errors
    /// Returns a PDF failure for an invalid page, raster dimensions or encoding.
    pub fn render_page(&self, page_number: usize) -> Result<PdfPage, AttachmentError> {
        let page = page_number
            .checked_sub(1)
            .and_then(|index| self.pdf.pages().get(index))
            .ok_or_else(|| AttachmentError::Pdf("Invalid PDF page".into()))?;
        let pixels = render_pdf_page(page, &RenderCache::new(), 1.5)
            .ok_or_else(|| AttachmentError::Pdf("Invalid PDF page dimensions".into()))?;
        let (width, height) = (pixels.width(), pixels.height());
        let png = pixels
            .into_png()
            .map_err(|error| AttachmentError::Pdf(error.to_string()))?;
        Ok(PdfPage { width, height, png })
    }
}

/// Extract ordered page text and an optional first-page PNG.
///
/// # Errors
/// Returns the native document-opening diagnostic.
pub(super) fn process_pdf(
    bytes: Vec<u8>,
    name: &str,
) -> Result<(String, Option<String>), AttachmentError> {
    let document = PdfDocument::new(bytes)?;
    let pdf = &document.pdf;
    let cache = InterpreterCache::new();
    let mut text = format!("<pdf filename=\"{name}\">");
    for (index, page) in pdf.pages().iter().enumerate() {
        let _ = write!(
            text,
            "\n<page number=\"{}\">\n{}\n</page>",
            index + 1,
            pdf_page_text(text_content::get_text_content(page, &cache))
        );
    }
    text.push_str("\n</pdf>");
    Ok((text, generate_pdf_preview(pdf)))
}

/// Fit the first page to 160 pixels, omitting failed PNG encodings.
fn generate_pdf_preview(pdf: &Pdf) -> Option<String> {
    let page = pdf.pages().first()?;
    let (width, height) = pdf_page_dimensions(page);
    let scale = (160.0 / width).min(160.0 / height);
    render_pdf_page(page, &RenderCache::new(), scale)?
        .into_png()
        .ok()
        .map(|bytes| STANDARD.encode(bytes))
}

/// Render one page at the supplied scale with the caller's document cache.
pub(crate) fn render_pdf_page<'a>(
    page: &'a Page<'a>,
    cache: &RenderCache<'a>,
    scale: f64,
) -> Option<Pixmap> {
    let (width, height) = pdf_page_dimensions(page);
    let width = (width * scale).to_u16()?;
    let height = (height * scale).to_u16()?;
    let mut context = RenderContext::new(width, height);
    hayro::render_into(
        page,
        cache,
        &InterpreterSettings::default(),
        &RenderSettings::default(),
        &mut context,
        Affine::scale(scale) * page.initial_transform(true).to_kurbo(),
    );
    context.flush();
    let mut pixmap = Pixmap::new(width, height);
    context.render_with(
        &mut pixmap,
        &mut Resources::default(),
        RasterizerSettings {
            target_init: TargetInit::Clear(WHITE),
            ..Default::default()
        },
    );
    Some(pixmap)
}

/// Join non-whitespace runs without trimming or deduplicating them.
fn pdf_page_text(runs: Vec<String>) -> String {
    runs.into_iter()
        .filter(|run| run.chars().any(|c| !maestro_text::is_whitespace_scalar(c)))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Read inherited page-box coordinates without narrowing the object numbers.
fn pdf_page_box(page: &Page<'_>, key: &[u8]) -> Option<PdfRect> {
    let mut parent: Option<Dict<'_>> = None;
    let mut seen = std::collections::HashSet::new();
    loop {
        let dict = parent.as_ref().unwrap_or_else(|| page.raw());
        if !seen.insert(dict.obj_id()) {
            return None;
        }
        if let Some(array) = dict.get::<Array<'_>>(key) {
            let mut numbers = array.iter::<f64>();
            let (x0, y0, x1, y1) = (
                numbers.next()?,
                numbers.next()?,
                numbers.next()?,
                numbers.next()?,
            );
            return Some(PdfRect::new(x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1)));
        }
        parent = Some(dict.get::<Dict<'_>>(b"Parent")?);
    }
}

/// Preserve box intersection and page rotation at full precision.
fn pdf_page_dimensions(page: &Page<'_>) -> (f64, f64) {
    let media = pdf_page_box(page, b"MediaBox").unwrap_or_else(|| page.media_box());
    let crop = pdf_page_box(page, b"CropBox")
        .unwrap_or(media)
        .intersect(media);
    let dimensions = (crop.width().max(1.0), crop.height().max(1.0));
    if crop.width() == 0.0 || crop.height() == 0.0 {
        return (
            f64::from(page.render_dimensions().0),
            f64::from(page.render_dimensions().1),
        );
    }
    match page.rotation() {
        Rotation::Horizontal | Rotation::FlippedHorizontal => (dimensions.1, dimensions.0),
        _ => dimensions,
    }
}

mod text_content;

#[cfg(test)]
mod tests;
