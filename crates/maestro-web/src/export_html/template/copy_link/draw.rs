//! Full-slot canvas interaction and selected-pack decoration.
use super::{CopyLinkState, build_share_url};
use crate::browser::{
    attachment_content::{self, channel, color},
    transition::{Transition, TransitionStyle},
};
use maestro_theme::BrandPresentation;
/// Borrowed activation and placement operands for one frame.
#[derive(Clone, Copy)]
pub struct CopyLinkView<'a> {
    /// Selected pack, resolved at draw time.
    pub presentation: &'a BrandPresentation<'a>,
    /// Active navigation branch's raw identity.
    pub current_leaf_id: &'a str,
    /// Current absolute browser location.
    pub location_href: &'a str,
    /// Optional literal injected base; an empty base remains distinct from absence.
    pub share_base_url: Option<&'a str>,
    /// Full native-measured control slot, before caller clipping.
    pub rect: egui::Rect,
    /// Real containing-message hover observation.
    pub message_hovered: bool,
}
/// Paint and activate one retained message-link control in the supplied slot.
/// # Errors
/// Returns reached URL preparation, glyph or selected-pack presentation errors.
pub fn render_copy_link_button(
    ui: &mut egui::Ui,
    entry_id: &str,
    state: &mut CopyLinkState,
    view: CopyLinkView<'_>,
) -> Result<egui::Response, std::io::Error> {
    let response = ui
        .interact(view.rect, state.id, egui::Sense::click())
        .on_hover_text("Copy link to this message")
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            response.enabled(),
            "Copy link to this message",
        )
    });
    let visible = view.rect.intersect(ui.clip_rect());
    let slots = if response.enabled() && visible.is_positive() {
        vec![((), visible, response.layer_id)]
    } else {
        vec![]
    };
    let events = ui.input(|input| input.events.clone());
    let pointer = !attachment_content::reading::controls::pointer_actions(
        &events,
        &mut state.pressed,
        &slots,
        ui.ctx(),
    )
    .is_empty();
    if pointer || (response.clicked() && !response.clicked_by(egui::PointerButton::Primary)) {
        state.copy(&build_share_url(entry_id, &view)?, ui.ctx());
    }
    paint(ui, state, &view, &response)?;
    Ok(response)
}
/// Animate the existing visual state without admitting an independent timer.
fn paint(
    ui: &mut egui::Ui,
    state: &mut CopyLinkState,
    view: &CopyLinkView<'_>,
    response: &egui::Response,
) -> Result<(), std::io::Error> {
    let copied = state.feedback.copied();
    let hovered = response.hovered() || response.has_focus();
    let operands = Operands::resolve(view, response, copied)?;
    let p = view.presentation;
    let border = color(
        p,
        if copied {
            "success"
        } else if hovered {
            "accent"
        } else {
            "border"
        },
    )?;
    let now = ui.input(|input| input.time);
    let sampled = state
        .transitions
        .get_or_insert_with(|| CopyLinkTransitions::new(&operands))
        .sample(now, &operands);
    if sampled.active {
        ui.ctx().request_repaint();
    }
    let Paint {
        opacity,
        background,
        color: foreground,
        ..
    } = sampled;
    if opacity <= 0. {
        return Ok(());
    }
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(view.rect));
    attachment_content::style(&mut child, p)?;
    child.set_opacity(opacity);
    child.painter().rect(
        view.rect,
        egui::CornerRadius::from(attachment_content::points(p.radii["control"])),
        background,
        egui::Stroke::new(1., border),
        egui::StrokeKind::Inside,
    );
    if copied {
        child.painter().text(
            view.rect.center(),
            egui::Align2::CENTER_CENTER,
            p.glyphs["passed"].symbol,
            egui::TextStyle::Button.resolve(child.style()),
            foreground,
        );
    } else {
        attachment_content::glyph(
            &child,
            egui::Rect::from_center_size(view.rect.center(), egui::vec2(14., 14.)),
            "Link",
            foreground,
        )
        .map_err(std::io::Error::other)?;
    }
    Ok(())
}

/// Resolved property destinations for the current frame.
struct Operands {
    /// Whole-control opacity.
    opacity: [f32; 1],
    /// Encoded-sRGB background components.
    background: [f32; 3],
    /// Encoded-sRGB foreground components.
    color: [f32; 3],
}
impl Operands {
    /// Resolve cascade priority before retaining property destinations.
    fn resolve(
        view: &CopyLinkView<'_>,
        response: &egui::Response,
        copied: bool,
    ) -> Result<Self, std::io::Error> {
        let hovered = response.hovered() || response.has_focus();
        let background = color(
            view.presentation,
            if copied {
                "success"
            } else if hovered {
                "accent"
            } else {
                "panel"
            },
        )?;
        let foreground = color(
            view.presentation,
            if copied || hovered {
                "on-accent"
            } else {
                "muted"
            },
        )?;
        Ok(Self {
            opacity: [if view.message_hovered { 1. } else { 0. }],
            background: [background.r(), background.g(), background.b()].map(f32::from),
            color: [foreground.r(), foreground.g(), foreground.b()].map(f32::from),
        })
    }
}
/// Three CSS properties retained by one copy-link mount.
pub(super) struct CopyLinkTransitions {
    /// Visibility run.
    opacity: Transition<1>,
    /// Background run.
    background: Transition<3>,
    /// Foreground run.
    color: Transition<3>,
}
impl CopyLinkTransitions {
    /// Mount at the resolved current destinations.
    fn new(operands: &Operands) -> Self {
        Self {
            opacity: Transition::new(operands.opacity),
            background: Transition::new(operands.background),
            color: Transition::new(operands.color),
        }
    }
    /// Sample all properties against one supplied frame instant.
    fn sample(&mut self, now: f64, target: &Operands) -> Paint {
        let style = TransitionStyle {
            duration: 0.15,
            curve: [0.25, 0.1, 0.25, 1.],
        };
        let opacity = self.opacity.sample(now, target.opacity, style);
        let background = self.background.sample(now, target.background, style);
        let foreground = self.color.sample(now, target.color, style);
        Paint {
            opacity: opacity.value[0],
            background: opaque_color(background.value),
            color: opaque_color(foreground.value),
            active: opacity.active || background.active || foreground.active,
        }
    }
}
/// Final paint operands, released from the retained transition borrow.
struct Paint {
    /// Whole-control opacity.
    opacity: f32,
    /// Quantized background.
    background: egui::Color32,
    /// Quantized foreground.
    color: egui::Color32,
    /// Whether to request another canvas frame.
    active: bool,
}
/// Encode an opaque sRGB value only at the paint boundary.
fn opaque_color(value: [f32; 3]) -> egui::Color32 {
    let [r, g, b] = value.map(channel);
    egui::Color32::from_rgb(r, g, b)
}
