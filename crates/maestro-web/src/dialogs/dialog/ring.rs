//! Fractional focus-shadow coverage rasterized before whole-control composition.
use crate::browser::attachment_content::AttachmentUiError;
use num_traits::ToPrimitive as _;
use resvg::tiny_skia::{FillRule, Mask, PathBuilder, Transform};

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
/// Supersampling factor per axis.
const SCALE: usize = 20;
/// `SCALE` as a float.
const SCALE_F: f32 = 20.;
/// One reusable texture per retained Close control.
#[derive(Default)]
pub(crate) struct Painter {
    /// Last raster operands and its pixel-aligned destination.
    cached: Option<(Ring, egui::Rect)>,
    /// Updated in place, released with its retained control.
    texture: Option<egui::TextureHandle>,
}
impl Painter {
    /// Rasterize changed operands only, then submit the same retained texture.
    pub(super) fn draw(&mut self, ui: &egui::Ui, ring: Ring) -> Result<(), AttachmentUiError> {
        if self.cached.as_ref().is_none_or(|(old, _)| *old != ring) {
            let bounds = ring.rect.expand(ring.spreads[1]);
            let pixels = egui::Rect::from_min_max(
                (bounds.min * ring.density).floor(),
                (bounds.max * ring.density).ceil(),
            );
            let image = raster(&ring, pixels)?;
            if let Some(texture) = &mut self.texture {
                texture.set(image, egui::TextureOptions::LINEAR);
            } else {
                self.texture = Some(ui.ctx().load_texture(
                    "close-focus",
                    image,
                    egui::TextureOptions::LINEAR,
                ));
            }
            self.cached = Some((ring, pixels / ring.density));
        }
        if let (Some(texture), Some((_, rect))) = (&self.texture, &self.cached) {
            ui.painter().image(
                texture.id(),
                *rect,
                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1., 1.)),
                egui::Color32::WHITE,
            );
        }
        Ok(())
    }
}
/// Rasterize shared boundaries once and subtract coverage before colour composition.
fn raster(ring: &Ring, pixels: egui::Rect) -> Result<egui::ColorImage, AttachmentUiError> {
    let base = mask(ring, pixels, 0.)?;
    let inner = mask(ring, pixels, ring.spreads[0])?;
    let outer = mask(ring, pixels, ring.spreads[1])?;
    let colors = ring.colors;
    let mut rgba = Vec::with_capacity(base.len() * 4);
    for ((base, inner), outer) in base.iter().zip(&inner).zip(&outer) {
        let coverage = [(inner - base).max(0.), (outer - inner).max(0.)];
        for (inner, outer) in colors[0].iter().zip(colors[1]) {
            let value = (inner * coverage[0] + outer * coverage[1]) / 255.;
            rgba.push(value.round().clamp(0., 255.).to_u8().unwrap_or_default());
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
/// Rounded-rectangle union filled in one pass, retaining fractional coordinates and radii.
fn mask(ring: &Ring, pixels: egui::Rect, spread: f32) -> Result<Vec<f32>, AttachmentUiError> {
    let rect = (ring.rect.expand(spread) * ring.density).translate(-pixels.min.to_vec2()) * SCALE_F;
    let radius = (ring.radius + spread) * ring.density * SCALE_F;
    let mut path = PathBuilder::new();
    for (x, y, width, height) in [
        (
            rect.left() + radius,
            rect.top(),
            rect.width() - 2. * radius,
            rect.height(),
        ),
        (
            rect.left(),
            rect.top() + radius,
            rect.width(),
            rect.height() - 2. * radius,
        ),
    ]
    .into_iter()
    .filter(|(_, _, width, height)| *width > 0. && *height > 0.)
    {
        path.push_rect(
            resvg::tiny_skia::Rect::from_xywh(x, y, width, height).ok_or_else(unavailable)?,
        );
    }
    for x in [rect.left() + radius, rect.right() - radius] {
        for y in [rect.top() + radius, rect.bottom() - radius] {
            path.push_circle(x, y, radius);
        }
    }
    let mut mask = Mask::new(
        (pixels.width() * SCALE_F)
            .to_u32()
            .ok_or_else(unavailable)?,
        (pixels.height() * SCALE_F)
            .to_u32()
            .ok_or_else(unavailable)?,
    )
    .ok_or_else(unavailable)?;
    mask.fill_path(
        &path.finish().ok_or_else(unavailable)?,
        FillRule::Winding,
        true,
        Transform::identity(),
    );
    Ok(downsample(&mask))
}
/// Unrepresentable raster geometry is reported through the existing host failure seam.
fn unavailable() -> AttachmentUiError {
    AttachmentUiError::Host("Unable to rasterize Close focus decoration".into())
}

/// Resolve tiny-skia's four-subpixel scan grid at 1/20 pixel before final colour encoding.
fn downsample(mask: &Mask) -> Vec<f32> {
    let width = usize::try_from(mask.width()).unwrap_or_default();
    let mut result = Vec::new();
    for rows in mask.data().chunks_exact(width * SCALE) {
        for x in (0..width).step_by(SCALE) {
            let sum: u32 = rows
                .chunks_exact(width)
                .flat_map(|row| &row[x..x + SCALE])
                .map(|v| u32::from(*v))
                .sum();
            result.push(sum.to_f32().unwrap_or_default() / (SCALE_F * SCALE_F));
        }
    }
    result
}
