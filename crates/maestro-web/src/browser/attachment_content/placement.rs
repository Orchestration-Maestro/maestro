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
    paint_clip_path(surface, surface.rect, css_pixels_per_point)
}

/// Clip reading decoration to finite canvas/ancestor bounds, not its viewport.
pub(super) fn frame_clip_path(surface: &AttachmentSurface<'_>, drawable: egui::Rect) -> String {
    let extent = if surface.rect.is_positive() {
        drawable
    } else {
        egui::Rect::NOTHING
    };
    paint_clip_path(surface, extent, (1., 1.))
}

/// Serialize the one outer extent and disjoint control holes in local CSS pixels.
fn paint_clip_path(
    surface: &AttachmentSurface<'_>,
    paint_rect: egui::Rect,
    css_pixels_per_point: (f64, f64),
) -> String {
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
    if path.is_empty() {
        // Empty CSS paths are invalid; a zero-area path keeps native clipping active.
        path.push_str("M0 0H0V0H0Z");
    }
    format!("path(evenodd,\"{path}\")")
}

/// Single canvas-to-CSS projection shared by placement and native readback.
pub(super) struct Transform {
    /// Viewport CSS origin.
    pub origin: [f64; 2],
    /// Independent CSS pixels per canvas point.
    pub scale: (f64, f64),
}
impl Transform {
    /// Derive scales from the actual drawable canvas.
    pub fn new(
        bounds: [f64; 4],
        physical: [u32; 2],
        pixels: f32,
    ) -> Result<Self, super::AttachmentHostError> {
        if physical.contains(&0) || bounds[2] <= 0. || bounds[3] <= 0. {
            return Err(super::AttachmentHostError::Host(
                "Canvas has no drawable size".into(),
            ));
        }
        Ok(Self {
            origin: [bounds[0], bounds[1]],
            scale: (
                bounds[2] * f64::from(pixels) / f64::from(physical[0]),
                bounds[3] * f64::from(pixels) / f64::from(physical[1]),
            ),
        })
    }
    /// Project a canvas point into viewport CSS pixels.
    pub fn position(&self, p: egui::Pos2) -> [f64; 2] {
        [
            self.origin[0] + f64::from(p.x) * self.scale.0,
            self.origin[1] + f64::from(p.y) * self.scale.1,
        ]
    }
    /// Invert a native viewport rectangle relative to a native wrapper origin.
    pub fn relative(&self, rect: [f64; 4], origin: [f64; 2]) -> egui::Rect {
        egui::Rect::from_min_size(
            egui::pos2(
                super::points((rect[0] - origin[0]) / self.scale.0),
                super::points((rect[1] - origin[1]) / self.scale.1),
            ),
            egui::vec2(super::points(rect[2] / self.scale.0), self.height(rect[3])),
        )
    }
    /// Read a natural CSS height in canvas points.
    pub fn height(&self, css: f64) -> f32 {
        super::points(css / self.scale.1)
    }
}
