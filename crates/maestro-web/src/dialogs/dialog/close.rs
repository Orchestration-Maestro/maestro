//! Mounted Close properties and non-overlapping focus-shadow paint.
use crate::browser::{
    attachment_content::{self as content, channel},
    transition::{Transition, TransitionStyle},
};
use crate::dialogs::dialog::DialogUiError;
use maestro_theme::BrandPresentation;

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
    /// Draw the glyph and disjoint annuli without multiplying overlap opacity twice.
    pub(crate) fn draw(
        &self,
        ui: &mut egui::Ui,
        close: egui::Rect,
        p: &BrandPresentation<'_>,
        painter: &mut super::ring::Painter,
    ) -> Result<(), DialogUiError> {
        if self.active {
            ui.ctx().request_repaint();
        }
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(close));
        child.set_opacity(self.opacity);
        content::glyph(
            &child,
            close,
            "X",
            egui::Color32::from_rgb(
                channel(self.color[0]),
                channel(self.color[1]),
                channel(self.color[2]),
            ),
        )?;
        let inner = self.shadow[4];
        let outer = self.shadow[9];
        if outer > 0. {
            let mut composed = [0.; 4];
            for (i, value) in composed.iter_mut().enumerate() {
                *value = self.shadow[i] + self.shadow[i + 5] * (1. - self.shadow[3]);
            }
            let radius = content::points(p.radii["control"] / 2.).min(close.width() / 2.);
            painter.draw(
                ui,
                super::ring::Ring {
                    rect: close,
                    radius,
                    spreads: [inner, outer],
                    colors: [
                        shadow_color(&composed, self.opacity),
                        shadow_color(&self.shadow[5..9], self.opacity),
                    ],
                    density: ui.ctx().pixels_per_point(),
                },
            )?;
        }
        Ok(())
    }
}
/// Scale premultiplied sRGB by whole-control opacity, alpha onto the channel range.
fn shadow_color(value: &[f32], opacity: f32) -> [f32; 4] {
    [
        value[0] * opacity,
        value[1] * opacity,
        value[2] * opacity,
        value[3] * opacity * 255.,
    ]
}
