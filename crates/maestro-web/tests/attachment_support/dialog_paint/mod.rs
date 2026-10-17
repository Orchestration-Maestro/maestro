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
    /// Integrate physical-pixel samples above the close control.
    /// # Panics
    /// Panics if controlled coordinates cannot be represented.
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
fn decoration(shapes: &[egui::epaint::ClippedShape]) -> Vec<egui::epaint::ClippedShape> {
    shapes
        .iter()
        .filter(|s| match &s.shape {
            egui::Shape::Rect(rect) => rect.rect.width() < 40.,
            egui::Shape::Mesh(mesh) => {
                mesh.calc_bounds().width() > 16. && mesh.calc_bounds().width() < 40.
            }
            _ => false,
        })
        .cloned()
        .collect()
}
