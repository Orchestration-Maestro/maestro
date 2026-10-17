//! Observe only the Close decoration after tessellation and texture sampling.
#![cfg(test)]
use num_traits::ToPrimitive as _;
use std::collections::HashMap;

/// Retained renderer textures, including in-place updates.
#[derive(Default)]
pub struct Paint {
    /// Texture data submitted by completed frames.
    textures: HashMap<egui::TextureId, egui::ColorImage>,
    /// Number of uploads for each retained texture.
    uploads: HashMap<egui::TextureId, usize>,
}
impl Paint {
    /// Apply full texture updates used by the control painter.
    pub fn update(&mut self, output: &egui::FullOutput) {
        for (id, delta) in output
            .textures_delta
            .set
            .iter()
            .flat_map(|(id, deltas)| deltas.iter().map(move |delta| (id, delta)))
        {
            if delta.pos.is_some() {
                continue;
            }
            let egui::ImageData::Color(image) = &delta.image;
            self.textures.insert(*id, (**image).clone());
            *self.uploads.entry(*id).or_default() += 1;
        }
        for id in &output.textures_delta.free {
            self.textures.remove(id);
        }
    }
    /// Upload count of the currently painted focus texture.
    #[must_use]
    pub fn ring_uploads(&self, shapes: &[egui::epaint::ClippedShape]) -> usize {
        shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Mesh(m) if decoration(&s.shape) => self.uploads.get(&m.texture_id),
                _ => None,
            })
            .sum()
    }
    /// Sample premultiplied alpha after the ring's actual triangle and texture paint.
    #[must_use]
    pub fn alpha(
        &self,
        ctx: &egui::Context,
        shapes: &[egui::epaint::ClippedShape],
        point: egui::Pos2,
    ) -> f32 {
        self.pixel(ctx, shapes, point)[3]
    }
    /// Sample premultiplied colour after the ring's actual triangle and texture paint.
    #[must_use]
    pub fn pixel(
        &self,
        ctx: &egui::Context,
        shapes: &[egui::epaint::ClippedShape],
        point: egui::Pos2,
    ) -> [f32; 4] {
        let shapes = shapes
            .iter()
            .filter(|s| decoration(&s.shape))
            .cloned()
            .collect();
        let mut pixel = [0.; 4];
        for primitive in ctx.tessellate(shapes, ctx.pixels_per_point()) {
            if !primitive.clip_rect.contains(point) {
                continue;
            }
            let egui::epaint::Primitive::Mesh(mesh) = primitive.primitive else {
                continue;
            };
            let source = mesh
                .indices
                .as_chunks::<3>()
                .0
                .iter()
                .find_map(|indices| triangle_pixel(&mesh, *indices, point, &self.textures));
            let Some(source) = source else {
                continue;
            };
            for (channel, value) in pixel.iter_mut().zip(source) {
                *channel = value + *channel * (1. - source[3] / 255.);
            }
        }
        pixel
    }
    /// Integrate the straight top edge in physical pixels, excluding the glyph.
    /// # Panics
    /// Panics if controlled coordinates cannot be represented as pixel indices.
    #[must_use]
    pub fn top_alpha(
        &self,
        ctx: &egui::Context,
        shapes: &[egui::epaint::ClippedShape],
        close: egui::Rect,
    ) -> f32 {
        let density = ctx.pixels_per_point();
        let top = (close.top() * density).floor().to_i32().unwrap();
        (top - 10..=top)
            .map(|y| {
                self.alpha(
                    ctx,
                    shapes,
                    egui::pos2(close.center().x, (y.to_f32().unwrap() + 0.5) / density),
                )
            })
            .sum::<f32>()
            / density
    }
}
/// Exclude the panel and sixteen-point glyph, retaining old stroke or raster decoration.
fn decoration(shape: &egui::Shape) -> bool {
    match shape {
        egui::Shape::Rect(rect) => rect.rect.width() < 40.,
        egui::Shape::Mesh(mesh) => {
            mesh.calc_bounds().width() > 16. && mesh.calc_bounds().width() < 40.
        }
        _ => false,
    }
}
/// Sample a covering triangle, interpolating the renderer's vertex colour and UV.
fn triangle_pixel(
    mesh: &egui::Mesh,
    indices: [u32; 3],
    point: egui::Pos2,
    textures: &HashMap<egui::TextureId, egui::ColorImage>,
) -> Option<[f32; 4]> {
    let [first, second, third] = indices.map(|i| mesh.vertices[usize::try_from(i).unwrap()]);
    let cross = |a: egui::Vec2, b: egui::Vec2| a.x * b.y - a.y * b.x;
    let area = cross(second.pos - first.pos, third.pos - first.pos);
    if area.abs() < f32::EPSILON {
        return None;
    }
    let weights = [
        cross(second.pos - point, third.pos - point) / area,
        cross(third.pos - point, first.pos - point) / area,
    ];
    let weights = [weights[0], weights[1], 1. - weights[0] - weights[1]];
    if weights.iter().any(|w| *w < 0.) {
        return None;
    }
    let vertices = [first, second, third];
    let mut source = std::array::from_fn(|i| {
        vertices
            .iter()
            .zip(weights)
            .map(|(v, w)| f32::from(v.color[i]) * w)
            .sum()
    });
    if let Some(image) = textures.get(&mesh.texture_id) {
        let uv: egui::Vec2 = vertices
            .iter()
            .zip(weights)
            .fold(egui::Vec2::ZERO, |uv, (v, w)| uv + v.uv.to_vec2() * w);
        for (channel, texture) in source.iter_mut().zip(texture_pixel(image, uv)) {
            *channel *= f32::from(texture) / 255.;
        }
    }
    Some(source)
}
/// Read the physical texel at the centre sampled by the submitted image mesh.
fn texture_pixel(image: &egui::ColorImage, uv: egui::Vec2) -> [u8; 4] {
    let index = |value: f32, size: usize| {
        (value * size.to_f32().unwrap())
            .floor()
            .to_usize()
            .unwrap()
            .min(size - 1)
    };
    image[(index(uv.x, image.size[0]), index(uv.y, image.size[1]))].to_array()
}
