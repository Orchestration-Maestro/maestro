//! Mounted Close properties and composed glyph/focus-shadow paint.
use crate::browser::control_paint::{self, Glyph, PaintBounds, Raster};
use crate::browser::{
    attachment_content::{self as content, channel},
    transition::{Transition, TransitionStyle},
};
use crate::dialogs::surface::DialogUiError;
use crate::icons::{self, IconSize};
use egui::emath::GuiRounding as _;
use maestro_theme::BrandPresentation;

/// Close-specific geometry and palette retained by the shared image owner.
#[derive(PartialEq)]
pub(crate) struct CloseKey {
    /// Native drawable allocation.
    bounds: PaintBounds,
    /// Fractional annulus policy.
    ring: super::ring::Ring,
    /// Opaque glyph role.
    color: egui::Color32,
}

/// Resolved CSS property endpoints, before quantization.
pub(crate) struct Operands {
    /// Whole-control opacity.
    opacity: [f32; 1],
    /// Opaque encoded-sRGB glyph channels.
    color: [f32; 3],
    /// Two premultiplied encoded-sRGB shadows, each with alpha and spread.
    shadow: [f32; 10],
}
impl Operands {
    /// Resolve fallible brand values before borrowing mounted state.
    pub(crate) fn resolve(
        p: &BrandPresentation<'_>,
        response: &egui::Response,
        native_focus: bool,
    ) -> Result<Self, DialogUiError> {
        let hover = response.hovered();
        let focus = response.has_focus() || native_focus;
        let color = content::color(p, if hover { "text" } else { "muted" })
            .map_err(|error| DialogUiError::Presentation(error.to_string()))?;
        let background = content::color(p, "background")
            .map_err(|error| DialogUiError::Presentation(error.to_string()))?;
        let ring = content::color(p, "accent")
            .map_err(|error| DialogUiError::Presentation(error.to_string()))?;
        let shadow = if focus {
            [
                f32::from(background.r()),
                f32::from(background.g()),
                f32::from(background.b()),
                1.,
                2.,
                f32::from(ring.r()),
                f32::from(ring.g()),
                f32::from(ring.b()),
                1.,
                4.,
            ]
        } else {
            [0.; 10]
        };
        Ok(Self {
            opacity: [if hover { 1. } else { 0.7 }],
            color: [
                f32::from(color.r()),
                f32::from(color.g()),
                f32::from(color.b()),
            ],
            shadow,
        })
    }
}
/// One independent run per CSS property, retained only while mounted.
pub(crate) struct CloseTransitions {
    /// Whole-control opacity track.
    opacity: Transition<1>,
    /// Glyph colour track, independent of opacity and focus.
    color: Transition<3>,
    /// The complete box-shadow property shares one reversing history.
    shadow: Transition<10>,
}
impl CloseTransitions {
    /// Mount at the current resolved style, without fictitious prior paint.
    pub(crate) fn new(operands: &Operands) -> Self {
        Self {
            opacity: Transition::new(operands.opacity),
            color: Transition::new(operands.color),
            shadow: Transition::new(operands.shadow),
        }
    }
    /// Sample all properties at the same frame time.
    pub(crate) fn sample(&mut self, now: f64, target: &Operands) -> Paint {
        let style = TransitionStyle {
            duration: 0.15,
            curve: [0.4, 0., 0.2, 1.],
        };
        let opacity = self.opacity.sample(now, target.opacity, style);
        let color = self.color.sample(now, target.color, style);
        let shadow = self.shadow.sample(now, target.shadow, style);
        Paint {
            opacity: opacity.value[0],
            color: color.value,
            shadow: shadow.value,
            active: opacity.active || color.active || shadow.active,
        }
    }
}
/// Sampled paint operands, no longer borrowing mounted state.
pub(crate) struct Paint {
    /// Opacity applied after shadow composition.
    opacity: f32,
    /// Encoded-sRGB glyph channels.
    color: [f32; 3],
    /// Premultiplied shadow components and spreads.
    shadow: [f32; 10],
    /// Combined repaint requirement.
    active: bool,
}
impl Paint {
    /// Compose the native glyph and disjoint annuli before fading once.
    pub(crate) fn draw(
        &self,
        ui: &mut egui::Ui,
        close: egui::Rect,
        p: &BrandPresentation<'_>,
        painter: &mut control_paint::Painter<CloseKey>,
    ) -> Result<(), DialogUiError> {
        if self.active {
            ui.ctx().request_repaint();
        }
        let glyph_rect = close.round_to_pixels(ui.ctx().pixels_per_point());
        let Some(bounds) =
            control_paint::raster_bounds(ui, close.expand(self.shadow[9]), glyph_rect)
        else {
            return Ok(());
        };
        let key = self.key(ui, close, p, bounds);
        painter.draw(ui, (key, self.opacity), |key| {
            let node = content::glyph_node("X")?;
            let image = icons::icon(&node, IconSize::Md, key.color, key.ring.density)
                .map_err(content::ControlError::Icon)?;
            let base = Raster {
                rect: key.bounds.rect,
                image: super::ring::raster(&key.ring, key.bounds.pixels)?,
            };
            Ok(control_paint::compose(
                ui,
                base,
                Glyph::Image {
                    image,
                    rect: glyph_rect,
                },
            )?)
        })
    }
    /// Retain annulus policy and raw colors separately from whole-control opacity.
    fn key(
        &self,
        ui: &egui::Ui,
        close: egui::Rect,
        p: &BrandPresentation<'_>,
        bounds: PaintBounds,
    ) -> CloseKey {
        let mut composed = [0.; 4];
        for (i, value) in composed.iter_mut().enumerate() {
            *value = self.shadow[i] + self.shadow[i + 5] * (1. - self.shadow[3]);
        }
        CloseKey {
            bounds,
            color: egui::Color32::from_rgb(
                channel(self.color[0]),
                channel(self.color[1]),
                channel(self.color[2]),
            ),
            ring: super::ring::Ring {
                rect: close,
                radius: content::points(p.radii["control"] / 2.),
                spreads: [self.shadow[4], self.shadow[9]],
                colors: [shadow_color(&composed), shadow_color(&self.shadow[5..9])],
                density: ui.ctx().pixels_per_point(),
            },
        }
    }
}
/// Put premultiplied shadow alpha onto the channel range without fading.
fn shadow_color(value: &[f32]) -> [f32; 4] {
    [value[0], value[1], value[2], value[3] * 255.]
}
