//! Native PDF page text and opaque rasterization.
use super::AttachmentError;
use base64::{Engine, engine::general_purpose::STANDARD};
use hayro::hayro_syntax::{
    Pdf,
    object::{Array, Dict, Rect as PdfRect},
    page::{Page, Rotation},
};
use hayro::kurbo::{Affine, BezPath, Rect};
use hayro::vello_cpu::color::palette::css::WHITE;
use hayro::vello_cpu::{Pixmap, RasterizerSettings, RenderContext, Resources, TargetInit};
use hayro::{RenderCache, RenderSettings};
use hayro_interpret::{
    BlendMode, ClipPath, Context, Device, DrawMode, DrawProps, Image, ImageDrawProps,
    InterpreterCache, InterpreterSettings, SoftMask, TransformExt, font::GlyphRun,
    hayro_cmap::BfString, interpret_page,
};
use num_traits::ToPrimitive;
use std::fmt::Write;

/// Extract ordered page text and an optional first-page PNG.
///
/// # Errors
/// Returns the native document-opening diagnostic.
pub(super) fn process_pdf(
    bytes: Vec<u8>,
    name: &str,
) -> Result<(String, Option<String>), AttachmentError> {
    let pdf = Pdf::new(bytes).map_err(|error| AttachmentError::Pdf(format!("{error:?}")))?;
    let cache = InterpreterCache::new();
    let mut text = format!("<pdf filename=\"{name}\">");
    for (index, page) in pdf.pages().iter().enumerate() {
        let (width, height) = page.render_dimensions();
        let settings = InterpreterSettings {
            render_annotations: false,
            ..InterpreterSettings::default()
        };
        let bounds = Rect::new(0.0, 0.0, f64::from(width), f64::from(height));
        let mut context = Context::new(Affine::IDENTITY, bounds, &cache, page.xref(), settings);
        let mut device = TextDevice::default();
        interpret_page(page, &mut context, &mut device);
        let _ = write!(
            text,
            "\n<page number=\"{}\">\n{}\n</page>",
            index + 1,
            pdf_page_text(device.runs)
        );
    }
    text.push_str("\n</pdf>");
    Ok((text, generate_pdf_preview(&pdf)))
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

/// Accumulate Unicode text once per decoded nonempty text show.
#[derive(Default)]
struct TextDevice {
    /// Interpreted runs in page order.
    runs: Vec<String>,
}
impl<'a> Device<'a> for TextDevice {
    fn draw_path(&mut self, _: &BezPath, _: DrawProps<'a>, _: &DrawMode) {}
    fn push_clip_path(&mut self, _: &ClipPath) {}
    fn push_transparency_group(&mut self, _: f32, _: Option<SoftMask<'a>>, _: BlendMode) {}
    fn draw_image(&mut self, _: Image<'a, '_>, _: ImageDrawProps<'a>) {}
    fn pop_clip(&mut self) {}
    fn pop_transparency_group(&mut self) {}
    fn draw_glyph_run(&mut self, _: &GlyphRun<'_, 'a>, _: DrawProps<'a>, _: &DrawMode) {}
    fn text_show(&mut self, run: &GlyphRun<'_, 'a>) {
        let text = run
            .glyphs()
            .iter()
            .filter_map(|glyph| glyph.as_unicode())
            .map(|value| match value {
                BfString::Char(c) => c.to_string(),
                BfString::String(value) => value,
            })
            .collect();
        self.runs.push(text);
    }
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

#[cfg(test)]
mod tests;
