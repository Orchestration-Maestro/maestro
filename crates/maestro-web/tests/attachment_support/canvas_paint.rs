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
    /// Images used by this frame's real image meshes, including retained uploads.
    #[must_use]
    pub fn images(&self, shapes: &[egui::epaint::ClippedShape]) -> Vec<egui::ColorImage> {
        shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Mesh(mesh) => self.textures.get(&mesh.texture_id).cloned(),
                _ => None,
            })
            .collect()
    }
    /// Upload count of the currently painted image textures.
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
    /// Sample premultiplied color after actual triangle and texture paint.
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
            for source in mesh
                .indices
                .as_chunks::<3>()
                .0
                .iter()
                .filter_map(|indices| triangle_pixel(&mesh, *indices, point, &self.textures))
            {
                pixel = std::array::from_fn(|i| source[i] + pixel[i] * (1. - source[3] / 255.));
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
        points(ctx, rect)
            .into_iter()
            .map(|point| {
                self.pixel(ctx, shapes, point)
                    .map(|c| c.round().to_u8().unwrap())
            })
            .collect()
    }

    /// Sample a native text producer independently of a composed control image.
    #[must_use]
    pub fn native_text(ctx: &egui::Context) -> Self {
        ctx.fonts(|fonts| Self::native_image(fonts.image()))
    }
    /// Sample an independently prepared native image through the same observer.
    #[must_use]
    pub fn native_image(image: egui::ColorImage) -> Self {
        Self {
            textures: HashMap::from([(egui::TextureId::default(), image)]),
            ..Self::default()
        }
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
        cross(second.pos - point, third.pos - point),
        cross(third.pos - point, first.pos - point),
        cross(first.pos - point, second.pos - point),
    ]
    .map(|edge| edge / area);
    if weights.iter().any(|w| *w < 0.) {
        return None;
    }
    // Shared triangle edges belong to one face, not both alpha-blended faces.
    for (weight, (from, to)) in
        weights
            .into_iter()
            .zip([(second, third), (third, first), (first, second)])
    {
        let edge = (to.pos - from.pos) * area.signum();
        if weight <= 0. && !(edge.y < 0. || edge.y <= 0. && edge.x > 0.) {
            return None;
        }
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
            *channel *= texture / 255.;
        }
    }
    Some(source)
}
/// Sample the production renderer's linear, clamp-to-edge texture lookup.
fn texture_pixel(image: &egui::ColorImage, uv: egui::Vec2) -> [f32; 4] {
    let axis = |value: f32, size: usize| {
        let value = (value * size.to_f32().unwrap() - 0.5).clamp(0., (size - 1).to_f32().unwrap());
        let low = value.floor().to_usize().unwrap();
        (low, (low + 1).min(size - 1), value.fract())
    };
    let (left, right, horizontal) = axis(uv.x, image.width());
    let (top, bottom, vertical) = axis(uv.y, image.height());
    let [top_left, top_right, bottom_left, bottom_right] =
        [(left, top), (right, top), (left, bottom), (right, bottom)]
            .map(|position| image[position].to_array().map(f32::from));
    std::array::from_fn(|channel| {
        let above = top_left[channel] + (top_right[channel] - top_left[channel]) * horizontal;
        let below =
            bottom_left[channel] + (bottom_right[channel] - bottom_left[channel]) * horizontal;
        above + (below - above) * vertical
    })
}
/// Recorded native texture and selected-role samples.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FilteringFixture {
    /// Recorded rows.
    pub rows: Vec<FilterRow>,
}
/// Recorded `FilterRow` observations.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FilterRow {
    /// Recorded size.
    pub size: [usize; 2],
    /// Recorded pixels.
    pub pixels: Vec<[u8; 4]>,
    /// Recorded uv.
    pub uv: [f32; 2],
    /// Recorded expected.
    pub expected: [f32; 4],
}
/// Read native filtering expectations shared by glyph observers.
/// # Panics
/// Panics if the committed native fixture has an invalid schema.
#[must_use]
pub fn filtering_fixture() -> FilteringFixture {
    serde_json::from_str(include_str!("../fixtures/control_texture_sampling.json")).unwrap()
}
#[test]
fn linear_texture_samples_match_native_filtering() {
    let valid: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/control_texture_sampling.json")).unwrap();
    crate::support::strict_record::<FilteringFixture>(&valid, &["rows"]);
    crate::support::strict_record::<FilterRow>(
        &valid["rows"][0],
        &["size", "pixels", "uv", "expected"],
    );
    for value in [serde_json::Value::Null, serde_json::json!([])] {
        let mut sibling = valid.clone();
        sibling["preview"] = value;
        assert!(serde_json::from_value::<FilteringFixture>(sibling).is_err());
    }
    let fixture = filtering_fixture();
    assert_eq!(fixture.rows.len(), 25);
    for (index, row) in fixture.rows.into_iter().enumerate() {
        let pixels = row
            .pixels
            .into_iter()
            .map(|[r, g, b, a]| egui::Color32::from_rgba_premultiplied(r, g, b, a))
            .collect();
        let image = egui::ColorImage::new(row.size, pixels);
        let actual = texture_pixel(&image, egui::vec2(row.uv[0], row.uv[1]));
        for (actual, expected) in actual.into_iter().zip(row.expected) {
            assert!(
                (actual - expected).abs() <= 0.5,
                "native filtering case {index}"
            );
        }
    }
}

/// Physical pixel centers within the measured region.
/// # Panics
/// Panics if controlled physical coordinates are not representable.
#[must_use]
pub fn points(ctx: &egui::Context, rect: egui::Rect) -> Vec<egui::Pos2> {
    let density = ctx.pixels_per_point();
    let left = (rect.left() * density).ceil().to_i32().unwrap();
    let top = (rect.top() * density).ceil().to_i32().unwrap();
    let right = (rect.right() * density).floor().to_i32().unwrap();
    let bottom = (rect.bottom() * density).floor().to_i32().unwrap();
    (top..bottom)
        .flat_map(|y| {
            (left..right).map(move |x| {
                egui::pos2(
                    (x.to_f32().unwrap() + 0.5) / density,
                    (y.to_f32().unwrap() + 0.5) / density,
                )
            })
        })
        .collect()
}

/// Original native image producer through the shared final-clip observer.
/// # Panics
/// Panics if the controlled glyph or density cannot be rasterized.
#[must_use]
pub fn icon(
    ctx: &egui::Context,
    rect: egui::Rect,
    name: &str,
    color: egui::Color32,
) -> (Paint, Vec<egui::epaint::ClippedShape>) {
    use egui::emath::GuiRounding as _;
    rotated_icon(
        ctx,
        rect.round_to_pixels(ctx.pixels_per_point()),
        name,
        (maestro_web::icons::IconSize::Md, color, 0.),
    )
}

/// Native glyph sampled after a caller-supplied reference rotation and size.
/// # Panics
/// Panics if the glyph cannot be resolved or rasterized.
#[must_use]
pub fn rotated_icon(
    ctx: &egui::Context,
    rect: egui::Rect,
    name: &str,
    (size, color, angle): (maestro_web::icons::IconSize, egui::Color32, f32),
) -> (Paint, Vec<egui::epaint::ClippedShape>) {
    let catalog = maestro_web::icons::icons().unwrap();
    let image =
        maestro_web::icons::icon(&catalog[name], size, color, ctx.pixels_per_point()).unwrap();
    let mut mesh = egui::Mesh::with_texture(egui::TextureId::default());
    mesh.add_rect_with_uv(
        rect,
        egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1., 1.)),
        egui::Color32::WHITE,
    );
    mesh.rotate(egui::emath::Rot2::from_angle(angle), rect.center());
    (
        Paint::native_image(image),
        vec![egui::epaint::ClippedShape {
            clip_rect: egui::Rect::EVERYTHING,
            shape: egui::Shape::mesh(mesh),
        }],
    )
}

#[cfg(test)]
mod tests {
    use super::Paint;

    /// Batched overlapping rectangles retain painter order after tessellation.
    #[test]
    fn canvas_samples_composite_every_covering_triangle_in_draw_order() {
        let context = egui::Context::default();
        let mut output = context.run_ui(egui::RawInput::default(), |ui| {
            let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(40., 40.));
            ui.painter().add(egui::Shape::Vec(vec![
                egui::Shape::rect_filled(rect, 0., egui::Color32::RED),
                egui::Shape::rect_filled(rect, 0., egui::Color32::BLUE),
            ]));
        });
        let mut paint = Paint::default();
        paint.update(&output);
        output.textures_delta.clear();
        assert_eq!(
            paint
                .pixel(&context, &output.shapes, egui::pos2(17., 19.))
                .map(f32::to_bits),
            [0.0_f32, 0., 255., 255.].map(f32::to_bits)
        );
    }
    /// One shared mesh edge cannot blend the same translucent surface twice.
    #[test]
    fn canvas_samples_shared_edges_only_once() {
        let context = egui::Context::default();
        let color = egui::Color32::from_rgba_premultiplied(0, 0, 128, 128);
        let mut output = context.run_ui(egui::RawInput::default(), |ui| {
            ui.painter().rect_filled(
                egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(40., 40.)),
                0.,
                color,
            );
        });
        let mut paint = Paint::default();
        paint.update(&output);
        output.textures_delta.clear();
        assert_eq!(
            paint
                .pixel(&context, &output.shapes, egui::pos2(20., 20.))
                .map(f32::to_bits),
            color.to_array().map(|v| f32::from(v).to_bits())
        );
    }
}
