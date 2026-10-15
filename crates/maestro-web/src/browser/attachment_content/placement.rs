//! Canvas-point clipping projected into element-local CSS pixels.
use super::AttachmentSurface;
use std::fmt::Write as _;
/// Clip canvas-point rectangles and return an element-local CSS-pixel path.
/// `paint_rect` and `rect` are canvas-point bounds; `left`/`top`/`right`/`bottom` are CSS pixels.
/// `css_pixels_per_point` supplies the horizontal and vertical canvas-to-CSS scales.
pub(super) fn clip_path(
    surface: &AttachmentSurface<'_>,
    css_pixels_per_point: (f64, f64),
) -> String {
    let paint_rect = if matches!(surface.content, super::AttachmentContent::Input { .. }) {
        let outset_css_pixels = 3.;
        let outset_points = egui::vec2(
            super::points(outset_css_pixels / css_pixels_per_point.0),
            super::points(outset_css_pixels / css_pixels_per_point.1),
        );
        surface.rect.expand2(outset_points)
    } else {
        surface.rect
    };
    let mut path = String::new();
    for rect in std::iter::once(surface.clip_rect.intersect(paint_rect)).chain(
        surface
            .occluders
            .iter()
            .map(|r| r.intersect(paint_rect).intersect(surface.clip_rect)),
    ) {
        if !rect.is_positive() {
            continue;
        }
        let left = f64::from(rect.left() - surface.rect.left()) * css_pixels_per_point.0;
        let top = f64::from(rect.top() - surface.rect.top()) * css_pixels_per_point.1;
        let right = f64::from(rect.right() - surface.rect.left()) * css_pixels_per_point.0;
        let bottom = f64::from(rect.bottom() - surface.rect.top()) * css_pixels_per_point.1;
        let _ = write!(path, "M{left} {top}H{right}V{bottom}H{left}Z");
    }
    format!("path(evenodd,\"{path}\")")
}
