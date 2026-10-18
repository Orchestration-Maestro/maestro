//! Retained native control pixels, composed before whole-control opacity.
use super::attachment_content::ControlError;
use egui::emath::GuiRounding as _;
use num_traits::ToPrimitive as _;
use resvg::tiny_skia::{FilterQuality, IntRect, IntSize, Pixmap, PixmapPaint, Transform};
use std::sync::Arc;

/// Owned pixels and their pixel-aligned logical destination.
pub(crate) struct Raster {
    /// Pixel-aligned logical destination.
    pub(crate) rect: egui::Rect,
    /// Owned premultiplied pixels.
    pub(crate) image: egui::ColorImage,
}
/// Drawable extent in logical and physical coordinates.
#[derive(Clone, Copy, PartialEq)]
pub(crate) struct PaintBounds {
    /// Pixel-aligned logical destination.
    pub(crate) rect: egui::Rect,
    /// Physical allocation bounds.
    pub(crate) pixels: egui::Rect,
}
/// Project decoration and glyph ink onto the native drawable viewport.
pub(crate) fn raster_bounds(
    ui: &egui::Ui,
    decoration: egui::Rect,
    glyph: egui::Rect,
) -> Option<PaintBounds> {
    let viewport = ui.ctx().viewport_rect();
    let viewport = ui
        .ctx()
        .layer_transform_from_global(ui.layer_id())
        .map_or(viewport, |transform| transform * viewport);
    let bounds = decoration.union(glyph).intersect(viewport);
    if !bounds.is_positive() {
        return None;
    }
    let density = ui.ctx().pixels_per_point();
    let pixels = egui::Rect::from_min_max(
        (bounds.min * density).floor(),
        (bounds.max * density).ceil(),
    );
    Some(PaintBounds {
        rect: pixels / density,
        pixels,
    })
}
/// The two glyph producers reached by these controls.
pub(crate) enum Glyph {
    /// Native SVG image.
    Image {
        /// Owned glyph pixels.
        image: egui::ColorImage,
        /// Unclipped target rectangle.
        rect: egui::Rect,
    },
    /// Toolkit-selected text layout.
    Text {
        /// Owned layout using the selected fonts.
        galley: Arc<egui::Galley>,
        /// Layout origin before native rounding.
        pos: egui::Pos2,
        /// Glyph tint.
        color: egui::Color32,
    },
}
/// Identity of the toolkit's retained glyph layout, invalidated by font replacement.
pub(crate) struct GlyphKey(
    /// Retained toolkit layout identity.
    pub(crate) Arc<egui::Galley>,
);
impl PartialEq for GlyphKey {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
/// One texture owner shared by both control callers.
pub(crate) struct Painter<K> {
    /// Last successfully prepared inputs and destination.
    cached: Option<(K, egui::Rect)>,
    /// Updated in place and released with the control.
    texture: Option<egui::TextureHandle>,
}
impl<K> Default for Painter<K> {
    fn default() -> Self {
        Self {
            cached: None,
            texture: None,
        }
    }
}
impl<K: PartialEq> Painter<K> {
    /// Prepare changed pixels before replacing the successful cache entry.
    pub(crate) fn draw<F, E>(
        &mut self,
        ui: &egui::Ui,
        (key, opacity): (K, f32),
        raster: F,
    ) -> Result<(), E>
    where
        F: FnOnce(&K) -> Result<Raster, E>,
    {
        if self.cached.as_ref().is_none_or(|(old, _)| *old != key) {
            let Raster { rect, image } = raster(&key)?;
            if let Some(texture) = &mut self.texture {
                texture.set(image, egui::TextureOptions::LINEAR);
            } else {
                self.texture = Some(ui.ctx().load_texture(
                    "control",
                    image,
                    egui::TextureOptions::LINEAR,
                ));
            }
            self.cached = Some((key, rect));
        }
        if let (Some(texture), Some((_, rect))) = (&self.texture, &self.cached) {
            ui.painter().image(
                texture.id(),
                *rect,
                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1., 1.)),
                egui::Color32::WHITE.gamma_multiply(opacity),
            );
        }
        Ok(())
    }
}
/// Report unrepresentable glyph geometry at the existing control boundary.
fn unavailable() -> ControlError {
    ControlError::Host("Unrepresentable icon canvas".into())
}
/// Consume toolkit pixels into the native raster library.
fn pixmap(image: egui::ColorImage) -> Result<Pixmap, ControlError> {
    let size = IntSize::from_wh(
        image.size[0].to_u32().ok_or_else(unavailable)?,
        image.size[1].to_u32().ok_or_else(unavailable)?,
    )
    .ok_or_else(unavailable)?;
    let data = image
        .pixels
        .into_iter()
        .flat_map(|c| c.to_array())
        .collect();
    Pixmap::from_vec(data, size).ok_or_else(unavailable)
}
/// Compose native glyph pixels before whole-control opacity.
pub(crate) fn compose(ui: &egui::Ui, base: Raster, glyph: Glyph) -> Result<Raster, ControlError> {
    let density = ui.ctx().pixels_per_point();
    let rect = base.rect;
    let mut target = pixmap(base.image)?;
    match glyph {
        Glyph::Image {
            image,
            rect: glyph_rect,
        } => {
            let source = pixmap(image)?;
            let glyph_rect = glyph_rect.round_to_pixels(density);
            blit(
                &mut target,
                &source,
                (glyph_rect.min - rect.min) * density,
                glyph_rect.size() * density,
            )?;
        }
        Glyph::Text { galley, pos, color } => text(ui, &mut target, (galley, pos, color), rect)?,
    }
    Ok(Raster {
        rect,
        image: egui::ColorImage::from_rgba_premultiplied(
            [
                target.width().to_usize().ok_or_else(unavailable)?,
                target.height().to_usize().ok_or_else(unavailable)?,
            ],
            target.data(),
        ),
    })
}
/// Delegate scaled glyph sampling and source-over to the native library.
fn blit(
    target: &mut Pixmap,
    source: &Pixmap,
    position: egui::Vec2,
    size: egui::Vec2,
) -> Result<(), ControlError> {
    target.draw_pixmap(
        0,
        0,
        source.as_ref(),
        &PixmapPaint {
            quality: FilterQuality::Bilinear,
            ..Default::default()
        },
        Transform::from_scale(
            size.x / source.width().to_f32().ok_or_else(unavailable)?,
            size.y / source.height().to_f32().ok_or_else(unavailable)?,
        )
        .post_translate(position.x, position.y),
        None,
    );
    Ok(())
}
/// Tint owned native glyph coverage and place it at the toolkit origin.
fn text(
    ui: &egui::Ui,
    target: &mut Pixmap,
    (galley, pos, color): (Arc<egui::Galley>, egui::Pos2, egui::Color32),
    bounds: egui::Rect,
) -> Result<(), ControlError> {
    let density = ui.ctx().pixels_per_point();
    // The toolkit provides an owned atlas snapshot; it is taken only for changed raster operands.
    let atlas = ui.ctx().fonts(|fonts| pixmap(fonts.image()))?;
    let origin = pos.round_to_pixels(density);
    for row in &galley.rows {
        for glyph in &row.glyphs {
            let uv = glyph.uv_rect;
            if uv.is_nothing() {
                continue;
            }
            let mut image = atlas
                .clone_rect(
                    IntRect::from_ltrb(
                        i32::from(uv.min[0]),
                        i32::from(uv.min[1]),
                        i32::from(uv.max[0]),
                        i32::from(uv.max[1]),
                    )
                    .ok_or_else(unavailable)?,
                )
                .ok_or_else(unavailable)?;
            let mut paint = resvg::tiny_skia::Paint::default();
            paint.set_color_rgba8(color.r(), color.g(), color.b(), color.a());
            paint.blend_mode = resvg::tiny_skia::BlendMode::SourceIn;
            image.fill_rect(
                resvg::tiny_skia::Rect::from_xywh(
                    0.,
                    0.,
                    image.width().to_f32().ok_or_else(unavailable)?,
                    image.height().to_f32().ok_or_else(unavailable)?,
                )
                .ok_or_else(unavailable)?,
                &paint,
                Transform::identity(),
                None,
            );
            let position =
                (origin - bounds.min + row.pos.to_vec2() + glyph.pos.to_vec2() + uv.offset)
                    * density;
            blit(target, &image, position, uv.size * density)?;
        }
    }
    Ok(())
}

/// Supersampling factor per axis.
const SCALE: usize = 20;
/// Supersampling factor as a float.
const SCALE_F: f32 = 20.;
/// Rasterize fractional rounded geometry with normalized equal radii.
pub(crate) fn rounded_mask(
    rect: egui::Rect,
    radius: f32,
    pixels: egui::Rect,
    density: f32,
) -> Option<Vec<f32>> {
    let radius = radius
        .max(0.)
        .min(rect.width() / 2.)
        .min(rect.height() / 2.);
    let rect = (rect * density).translate(-pixels.min.to_vec2()) * SCALE_F;
    let radius = radius * density * SCALE_F;
    let mut path = resvg::tiny_skia::PathBuilder::new();
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
        path.push_rect(resvg::tiny_skia::Rect::from_xywh(x, y, width, height)?);
    }
    for x in [rect.left() + radius, rect.right() - radius] {
        for y in [rect.top() + radius, rect.bottom() - radius] {
            path.push_circle(x, y, radius);
        }
    }
    let mut mask = resvg::tiny_skia::Mask::new(
        (pixels.width() * SCALE_F).to_u32()?,
        (pixels.height() * SCALE_F).to_u32()?,
    )?;
    mask.fill_path(
        &path.finish()?,
        resvg::tiny_skia::FillRule::Winding,
        true,
        Transform::identity(),
    );
    Some(downsample(&mask))
}
/// Subtract shared coverage without negative seams.
pub(crate) fn coverage_difference(outer: &[f32], inner: &[f32]) -> Vec<f32> {
    outer
        .iter()
        .zip(inner)
        .map(|(outside, inside)| (outside - inside).max(0.))
        .collect()
}
/// Resolve the native subpixel scan grid before final color encoding.
fn downsample(mask: &resvg::tiny_skia::Mask) -> Vec<f32> {
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

#[cfg(test)]
mod tests;
