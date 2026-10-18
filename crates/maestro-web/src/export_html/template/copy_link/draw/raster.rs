//! Opaque copy-link decoration and native image preparation.
use super::{CopyKey, Glyph};
use crate::{
    browser::{
        attachment_content::ControlError,
        control_paint::{self, Raster},
    },
    icons::{self, IconSize},
};
use num_traits::ToPrimitive as _;

/// Resolve the existing catalog glyph only on a reached raster miss.
pub(super) fn link(key: &CopyKey) -> Result<Glyph, ControlError> {
    let node = crate::browser::attachment_content::glyph_node("Link")?;
    Ok(Glyph::Image {
        image: icons::icon(&node, IconSize::Md, key.color, key.density)?,
        rect: egui::Rect::from_center_size(key.rect.center(), egui::vec2(14., 14.)),
    })
}
/// Fill the complete background before source-overing its inside border.
pub(super) fn background(ui: &egui::Ui, key: &CopyKey) -> Result<Raster, ControlError> {
    let unavailable = || ControlError::Host("Unrepresentable icon canvas".into());
    let outer = control_paint::rounded_mask(key.rect, key.radius, key.bounds.pixels, key.density)
        .ok_or_else(unavailable)?;
    let inner = control_paint::rounded_mask(
        key.rect.shrink(1.),
        key.radius - 1.,
        key.bounds.pixels,
        key.density,
    )
    .ok_or_else(unavailable)?;
    let border = control_paint::coverage_difference(&outer, &inner);
    let size = [
        key.bounds
            .pixels
            .width()
            .to_usize()
            .ok_or_else(unavailable)?,
        key.bounds
            .pixels
            .height()
            .to_usize()
            .ok_or_else(unavailable)?,
    ];
    let image = |coverage: Vec<f32>, color: egui::Color32| {
        egui::ColorImage::new(
            size,
            coverage
                .into_iter()
                .map(|alpha| color.gamma_multiply(alpha / 255.))
                .collect(),
        )
    };
    control_paint::compose(
        ui,
        Raster {
            rect: key.bounds.rect,
            image: image(outer, key.background),
        },
        Glyph::Image {
            image: image(border, key.border),
            rect: key.bounds.rect,
        },
    )
}
