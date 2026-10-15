//! CSS clipping in the full native element's coordinate space.
use super::AttachmentSurface;
use std::fmt::Write as _;
/// Preserve the image layout while cutting out canvas-painted controls.
pub(super) fn clip_path(surface: &AttachmentSurface<'_>, scale: (f64, f64)) -> String {
    let paint_rect = if matches!(surface.content, super::AttachmentContent::Input { .. }) {
        surface.rect.expand(3.)
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
        let left = f64::from(rect.left() - surface.rect.left()) * scale.0;
        let top = f64::from(rect.top() - surface.rect.top()) * scale.1;
        let right = f64::from(rect.right() - surface.rect.left()) * scale.0;
        let bottom = f64::from(rect.bottom() - surface.rect.top()) * scale.1;
        let _ = write!(path, "M{left} {top}H{right}V{bottom}H{left}Z");
    }
    format!("path(evenodd,\"{path}\")")
}
