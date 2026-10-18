//! Observe the dialog decoration through the shared final-clip raster sampler.
#![cfg(test)]
use num_traits::ToPrimitive as _;

#[derive(Default)]
pub struct Paint(super::canvas_paint::Paint);
impl Paint {
    pub fn update(&mut self, output: &egui::FullOutput) {
        self.0.update(output);
    }
    #[must_use]
    pub fn ring_uploads(&self, shapes: &[egui::epaint::ClippedShape]) -> usize {
        self.0.uploads(&decoration(shapes))
    }
    #[must_use]
    pub fn alpha(
        &self,
        ctx: &egui::Context,
        shapes: &[egui::epaint::ClippedShape],
        point: egui::Pos2,
    ) -> f32 {
        self.pixel(ctx, shapes, point)[3]
    }
    #[must_use]
    pub fn pixel(
        &self,
        ctx: &egui::Context,
        shapes: &[egui::epaint::ClippedShape],
        point: egui::Pos2,
    ) -> [f32; 4] {
        self.0.pixel(ctx, &decoration(shapes), point)
    }
    /// Read the painted physical-pixel samples above the close control.
    /// # Panics
    /// Panics if controlled coordinates cannot be represented.
    #[must_use]
    pub fn top_rows(
        &self,
        ctx: &egui::Context,
        shapes: &[egui::epaint::ClippedShape],
        close: egui::Rect,
    ) -> Vec<f32> {
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
            .filter(|a| *a > 0.)
            .collect()
    }
    /// Integrate physical-pixel samples above the close control.
    #[must_use]
    pub fn top_alpha(
        &self,
        ctx: &egui::Context,
        shapes: &[egui::epaint::ClippedShape],
        close: egui::Rect,
    ) -> f32 {
        self.top_rows(ctx, shapes, close).into_iter().sum::<f32>() / ctx.pixels_per_point()
    }
}
/// Compare integrated pixel opacity to independent fractional shadow coverage.
/// # Panics
/// Panics if the painted coverage differs from the spread's coverage.
pub fn assert_spread(actual: f32, spread: f32, opacity: f32) {
    let progress = spread / 2.;
    let expected = spread * opacity * 255. * progress * (3. - progress);
    assert!(
        (actual - expected).abs() < 3.,
        "painted coverage {actual}, expected {expected} for spread {spread}"
    );
}
/// Select the sixteen-point Close glyph meshes.
pub fn glyphs(shapes: &[egui::epaint::ClippedShape]) -> impl Iterator<Item = &egui::epaint::Mesh> {
    shapes.iter().filter_map(|s| match &s.shape {
        egui::Shape::Mesh(mesh) if (mesh.calc_bounds().width() - 16.).abs() < 0.001 => {
            Some(&**mesh)
        }
        _ => None,
    })
}
/// Exclude the panel and sixteen-point glyph, retaining the raster ring decoration.
fn decoration(shapes: &[egui::epaint::ClippedShape]) -> Vec<egui::epaint::ClippedShape> {
    shapes
        .iter()
        .filter(|s| {
            matches!(&s.shape, egui::Shape::Mesh(mesh)
                if mesh.calc_bounds().width() > 16. && mesh.calc_bounds().width() < 40.)
        })
        .cloned()
        .collect()
}
