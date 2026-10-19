//! Observe the dialog decoration through the shared final-clip raster sampler.
#![cfg(test)]
use num_traits::ToPrimitive as _;

#[derive(Default)]
pub struct Paint(super::canvas_paint::Paint);
impl Paint {
    pub fn update(&mut self, output: &egui::FullOutput) {
        self.0.update(output);
    }
    /// Raster images actually used by the frame's image meshes.
    #[must_use]
    pub fn images(&self, shapes: &[egui::epaint::ClippedShape]) -> Vec<egui::ColorImage> {
        self.0.images(shapes)
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
    let opaque = [
        (255. * progress * (2. - progress)).round(),
        (255. * progress).round(),
    ];
    let vertex = f32::from(egui::Color32::WHITE.gamma_multiply(opacity).a()) / 255.;
    let expected = spread * vertex * (opaque[0] + opaque[1]);
    assert!(
        (actual - expected).abs() < 3.,
        "painted coverage {actual}, expected {expected} for spread {spread}"
    );
}
/// Select the composed Close texture, including its optional annuli.
pub fn glyphs(shapes: &[egui::epaint::ClippedShape]) -> impl Iterator<Item = &egui::epaint::Mesh> {
    shapes.iter().filter_map(|s| match &s.shape {
        egui::Shape::Mesh(mesh) if (0.001..40.).contains(&mesh.calc_bounds().width()) => {
            Some(&**mesh)
        }
        _ => None,
    })
}
/// Exclude the panel, retaining the composed Close decoration.
fn decoration(shapes: &[egui::epaint::ClippedShape]) -> Vec<egui::epaint::ClippedShape> {
    shapes
        .iter()
        .filter(|s| {
            matches!(&s.shape, egui::Shape::Mesh(mesh)
                if mesh.calc_bounds().width() > 0. && mesh.calc_bounds().width() < 40.)
        })
        .cloned()
        .collect()
}
