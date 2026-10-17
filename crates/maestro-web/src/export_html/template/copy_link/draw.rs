//! Full-slot canvas interaction and selected-pack decoration.
use super::{CopyLinkState, build_share_url};
use crate::browser::attachment_content::{self, AttachmentUiError};
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
) -> Result<egui::Response, AttachmentUiError> {
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
    state: &CopyLinkState,
    view: &CopyLinkView<'_>,
    response: &egui::Response,
) -> Result<(), AttachmentUiError> {
    let copied = state.feedback.borrow().is_some();
    let hovered = response.hovered() || response.has_focus();
    let opacity =
        ui.ctx()
            .animate_bool_with_time(state.id.with("visibility"), view.message_hovered, 0.15);
    let accent = ui
        .ctx()
        .animate_bool_with_time(state.id.with("hover"), hovered, 0.15);
    let success = ui
        .ctx()
        .animate_bool_with_time(state.id.with("success"), copied, 0.15);
    let p = view.presentation;
    let border_role = if copied {
        "success"
    } else if hovered {
        "accent"
    } else {
        "border"
    };
    let (background, border, foreground) = colors(p, accent, success, border_role)?;
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
        )?;
    }
    Ok(())
}

/// Resolve animated panel/glyph colors and the immediate boundary role.
fn colors(
    p: &BrandPresentation<'_>,
    accent: f32,
    success: f32,
    border_role: &str,
) -> Result<(egui::Color32, egui::Color32, egui::Color32), AttachmentUiError> {
    let mix = |a: egui::Color32, b: egui::Color32, t: f32| -> egui::Color32 {
        egui::lerp(egui::Rgba::from(a)..=egui::Rgba::from(b), t).into()
    };
    let background = mix(
        attachment_content::color(p, "panel")?,
        attachment_content::color(p, "accent")?,
        accent,
    );
    let background = mix(
        background,
        attachment_content::color(p, "success")?,
        success,
    );
    let border = attachment_content::color(p, border_role)?;
    let foreground = mix(
        attachment_content::color(p, "muted")?,
        attachment_content::color(p, "on-accent")?,
        accent.max(success),
    );
    Ok((background, border, foreground))
}
