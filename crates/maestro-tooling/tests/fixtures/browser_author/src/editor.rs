//! Native-testable widget presentation for the editable author fixture.
use super::egui;

/// Render the single author input.
pub fn show(ui: &mut egui::Ui, text: &mut String) -> egui::Response {
    let id = ui.make_persistent_id("author-input");
    let radius = ui.visuals().widgets.inactive.corner_radius;
    let frame = egui::Frame::new()
        .inner_margin(egui::Margin::symmetric(4, 2))
        .fill(ui.visuals().extreme_bg_color)
        .corner_radius(radius);
    let response = ui.add(egui::TextEdit::singleline(text).id(id).frame(frame));
    let stroke = if response.has_focus() {
        egui::Stroke::new(2.0, ui.visuals().selection.bg_fill)
    } else {
        ui.visuals().widgets.inactive.bg_stroke
    };
    ui.painter()
        .rect_stroke(response.rect, radius, stroke, egui::StrokeKind::Outside);
    response
}

/// Project the six shared color roles into toolkit visuals.
pub fn visuals(
    [text, background, panel, border, accent, on_accent]: [egui::Color32; 6],
) -> egui::Visuals {
    let mut visuals = egui::Visuals::dark();
    visuals.override_text_color = Some(text);
    visuals.panel_fill = background;
    visuals.extreme_bg_color = panel;
    visuals.selection.bg_fill = accent;
    visuals.selection.stroke.color = on_accent;
    visuals.text_cursor.stroke.color = text;
    visuals.ime_composition.active_underline_stroke.color = text;
    visuals.ime_composition.inactive_underline_stroke.color = text;
    for widget in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.bg_fill = panel;
        widget.weak_bg_fill = panel;
        widget.fg_stroke.color = text;
        widget.bg_stroke.color = border;
    }
    visuals
}
