//! Texture-backed samples after tessellation and the final clip.
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
    /// Textures released after the preceding frame was painted.
    free: Vec<egui::TextureId>,
}
impl Paint {
    /// Apply full texture updates used by the control painter.
    /// # Panics
    /// Panics if the renderer updates a texture before allocating it.
    pub fn update(&mut self, output: &egui::FullOutput) {
        for id in self.free.drain(..) {
            self.textures.remove(&id);
        }
        for (id, delta) in output
            .textures_delta
            .set
            .iter()
            .flat_map(|(id, deltas)| deltas.iter().map(move |delta| (id, delta)))
        {
            let egui::ImageData::Color(image) = &delta.image;
            self.upload(*id, image, delta.pos);
            *self.uploads.entry(*id).or_default() += 1;
        }
        self.free.extend(output.textures_delta.free.iter().copied());
    }
    /// Apply a full image or a partial texture update before sampling the frame.
    fn upload(&mut self, id: egui::TextureId, image: &egui::ColorImage, pos: Option<[usize; 2]>) {
        let Some([x, y]) = pos else {
            self.textures.insert(id, image.clone());
            return;
        };
        let previous = self
            .textures
            .get_mut(&id)
            .expect("a partial upload follows allocation");
        for (row, pixels) in image.pixels.chunks(image.width()).enumerate() {
            let start = (y + row) * previous.width() + x;
            previous.pixels[start..start + pixels.len()].copy_from_slice(pixels);
        }
    }
    /// Upload count of the currently painted focus texture.
    #[must_use]
    pub fn uploads(&self, shapes: &[egui::epaint::ClippedShape]) -> usize {
        shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::Shape::Mesh(m) => self.uploads.get(&m.texture_id),
                _ => None,
            })
            .sum()
    }
    /// Sample premultiplied colour after the ring's actual triangle and texture paint.
    #[must_use]
    pub fn pixel(
        &self,
        ctx: &egui::Context,
        shapes: &[egui::epaint::ClippedShape],
        point: egui::Pos2,
    ) -> [f32; 4] {
        let mut pixel = [0.; 4];
        for primitive in shapes
            .iter()
            .flat_map(|shape| ctx.tessellate(vec![shape.clone()], ctx.pixels_per_point()))
        {
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
    /// Visible painted pixels in a measured region, at physical pixel centers.
    /// # Panics
    /// Panics if controlled coordinates or colors cannot be represented.
    #[must_use]
    pub fn region(
        &self,
        ctx: &egui::Context,
        shapes: &[egui::epaint::ClippedShape],
        rect: egui::Rect,
    ) -> Vec<[u8; 4]> {
        let density = ctx.pixels_per_point();
        let left = (rect.left() * density).ceil().to_i32().unwrap();
        let top = (rect.top() * density).ceil().to_i32().unwrap();
        let right = (rect.right() * density).floor().to_i32().unwrap();
        let bottom = (rect.bottom() * density).floor().to_i32().unwrap();
        (top..bottom)
            .flat_map(|y| (left..right).map(move |x| (x, y)))
            .map(|(x, y)| {
                self.pixel(
                    ctx,
                    shapes,
                    egui::pos2(
                        (x.to_f32().unwrap() + 0.5) / density,
                        (y.to_f32().unwrap() + 0.5) / density,
                    ),
                )
                .map(|c| c.round().to_u8().unwrap())
            })
            .collect()
    }
    /// Text counts only if its glyphs survive tessellation, texture sampling and clipping.
    #[must_use]
    pub fn text_visible(
        &self,
        ctx: &egui::Context,
        shapes: &[egui::epaint::ClippedShape],
        text: &str,
    ) -> bool {
        shapes
            .iter()
            .filter(|shape| matches!(&shape.shape, egui::Shape::Text(t) if t.galley.text() == text))
            .any(|shape| {
                self.region(
                    ctx,
                    std::slice::from_ref(shape),
                    shape
                        .shape
                        .visual_bounding_rect()
                        .intersect(shape.clip_rect),
                )
                .iter()
                .any(|p| p[3] > 0)
            })
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
    {
        let image = textures.get(&mesh.texture_id)?;
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
