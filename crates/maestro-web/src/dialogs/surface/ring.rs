//! Fractional focus-shadow coverage rasterized before whole-control composition.
use crate::browser::attachment_content::channel;
use crate::browser::control_paint::{coverage_difference, rounded_mask};
use crate::dialogs::surface::DialogUiError;
use num_traits::ToPrimitive as _;

/// Resolved geometry and premultiplied annulus colours.
#[derive(Clone, Copy, PartialEq)]
pub(super) struct Ring {
    /// Unexpanded control boundary.
    pub rect: egui::Rect,
    /// Fractional unexpanded corner radius.
    pub radius: f32,
    /// Inner and outer shadow spread.
    pub spreads: [f32; 2],
    /// Composited inner and independent outer paint.
    pub colors: [[f32; 4]; 2],
    /// Physical pixels per logical point.
    pub density: f32,
}
/// Rasterize shared boundaries once and subtract coverage before colour composition.
pub(super) fn raster(ring: &Ring, pixels: egui::Rect) -> Result<egui::ColorImage, DialogUiError> {
    let base = mask(ring, pixels, 0.)?;
    let inner = mask(ring, pixels, ring.spreads[0])?;
    let outer = mask(ring, pixels, ring.spreads[1])?;
    let colors = ring.colors;
    let mut rgba = Vec::with_capacity(base.len() * 4);
    let first = coverage_difference(&inner, &base);
    let second = coverage_difference(&outer, &inner);
    for (inner, outer) in first.into_iter().zip(second) {
        let coverage = [inner, outer];
        for (inner, outer) in colors[0].iter().zip(colors[1]) {
            let value = (inner * coverage[0] + outer * coverage[1]) / 255.;
            rgba.push(channel(value));
        }
    }
    Ok(egui::ColorImage::from_rgba_premultiplied(
        [
            pixels.width().to_usize().ok_or_else(unavailable)?,
            pixels.height().to_usize().ok_or_else(unavailable)?,
        ],
        &rgba,
    ))
}
/// Adapt shared native mask failure to the Close-specific boundary.
fn mask(ring: &Ring, pixels: egui::Rect, spread: f32) -> Result<Vec<f32>, DialogUiError> {
    rounded_mask(
        ring.rect.expand(spread),
        ring.radius + spread,
        pixels,
        ring.density,
    )
    .ok_or_else(unavailable)
}
/// Unrepresentable focus geometry keeps its existing host error.
fn unavailable() -> DialogUiError {
    DialogUiError::Host(
        crate::browser::attachment_content::AttachmentHostError::Host(
            "Unable to rasterize Close focus decoration".into(),
        ),
    )
}
