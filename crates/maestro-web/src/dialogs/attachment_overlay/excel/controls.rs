//! Canvas sheet tabs occupying measured native slots.
use super::Excel;
use crate::browser::attachment_content::{
    AttachmentContent, AttachmentHost, AttachmentHostError, AttachmentSurface, ReadingPhase,
    ReadingPlacement, color, points, reading::controls::pointer_actions, style,
};
/// Measure, publish and activate one retained workbook's sheet controls.
pub(super) fn present(
    excel: &mut Excel,
    host: &dyn AttachmentHost,
    ui: &mut egui::Ui,
    surface: (egui::Id, egui::Rect, Option<egui::Id>, bool),
    presentation: &maestro_theme::BrandPresentation<'_>,
) -> Result<(), AttachmentHostError> {
    let mut body = ui.new_child(egui::UiBuilder::new().max_rect(surface.1));
    let ui = &mut body;
    style(ui, presentation)
        .map_err(|error| AttachmentHostError::Presentation(error.to_string()))?;
    ui.visuals_mut().widgets.inactive.fg_stroke.color = color(presentation, "muted")
        .map_err(|error| AttachmentHostError::Presentation(error.to_string()))?;
    ui.visuals_mut().selection.stroke.color = color(presentation, "accent")
        .map_err(|error| AttachmentHostError::Presentation(error.to_string()))?;
    if let Some(font) = ui.style_mut().text_styles.get_mut(&egui::TextStyle::Button) {
        font.size *= 0.875;
    }
    ui.spacing_mut().button_padding = egui::vec2(
        points(presentation.spacing["md"]),
        points(presentation.spacing["sm"]),
    );
    let sizes = sizes(excel, ui, surface.0);
    let Some(reading) = &excel.reading else {
        return Ok(());
    };
    let place = |phase, holes| {
        host.present(
            AttachmentSurface {
                id: surface.0,
                rect: surface.1,
                clip_rect: ui.clip_rect(),
                pixels_per_point: ui.ctx().pixels_per_point(),
                opacity: 1.,
                occluders: holes,
                content: AttachmentContent::Reading(ReadingPlacement {
                    intrinsic_width: false,
                    content: reading.clone(),
                    phase,
                    sizes: &sizes,
                    modal: surface.2,
                    context: ui.ctx(),
                }),
            },
            presentation,
        )
    };
    place(ReadingPhase::Measure, vec![])?;
    let controls = if sizes.is_empty() {
        vec![]
    } else {
        host.reading_layout(surface.0)?.controls
    };
    let controls: Vec<_> = controls
        .into_iter()
        .map(|(id, rect)| (id, rect.translate(surface.1.min.to_vec2())))
        .collect();
    place(
        ReadingPhase::Publish,
        controls.iter().map(|(_, r)| *r).collect(),
    )?;
    draw_tabs(excel, ui, (surface.0, surface.1, &controls), presentation)
}
/// Paint each full tab while admitting gestures only in its visible intersection.
fn draw_tabs(
    excel: &mut Excel,
    ui: &mut egui::Ui,
    placement: (egui::Id, egui::Rect, &[(egui::Id, egui::Rect)]),
    presentation: &maestro_theme::BrandPresentation<'_>,
) -> Result<(), AttachmentHostError> {
    let (identity, rect, controls) = placement;
    let mut slots = Vec::new();
    for (index, (label, _)) in excel
        .sheets
        .values
        .iter()
        .enumerate()
        .filter(|_| excel.sheets.values.len() > 1)
    {
        let id = identity.with(index);
        let Some((_, full)) = controls.iter().find(|(slot, _)| *slot == id) else {
            continue;
        };
        let visible = full.intersect(rect).intersect(ui.clip_rect());
        let mut child = ui.new_child(egui::UiBuilder::new().id_salt(id).max_rect(*full));
        child.set_clip_rect(visible);
        let selected = index == excel.selected.get();
        let response = child.put(*full, button(label, selected));
        #[cfg(target_arch = "wasm32")]
        if response.gained_focus() {
            super::browser::reveal(&excel.sheets, index)?;
            ui.ctx().request_repaint();
        }
        let active = selected || response.hovered() || response.has_focus();
        if active {
            let ink = color(presentation, if selected { "accent" } else { "border" })
                .map_err(|error| AttachmentHostError::Presentation(error.to_string()))?;
            child.painter().hline(
                full.x_range(),
                full.bottom() - 1.,
                egui::Stroke::new(2., ink),
            );
        }
        if response.enabled() && visible.is_positive() {
            slots.push((index, visible, response.layer_id));
        }
        if response.clicked() && !response.clicked_by(egui::PointerButton::Primary) {
            excel.selected.set(index);
        }
    }
    let events = ui.input(|input| input.events.clone());
    for index in pointer_actions(&events, &mut excel.pressed, &slots, ui.ctx()) {
        excel.selected.set(index);
    }
    Ok(())
}
/// Use one tab widget for measurement and full-slot painting.
fn button(label: &str, selected: bool) -> egui::Button<'_> {
    egui::Button::new(label)
        .selected(selected)
        .frame(true)
        .fill(egui::Color32::TRANSPARENT)
        .stroke(egui::Stroke::NONE)
        .corner_radius(0.)
        .wrap_mode(egui::TextWrapMode::Extend)
}
/// Measure tabs without painting, input admission or parent allocation.
fn sizes(excel: &Excel, ui: &mut egui::Ui, id: egui::Id) -> Vec<(egui::Id, egui::Vec2)> {
    if excel.sheets.values.len() <= 1 {
        return vec![];
    }
    excel
        .sheets
        .values
        .iter()
        .enumerate()
        .map(|(index, (name, _))| {
            let mut child = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt((id, index, "measure"))
                    .invisible()
                    .sizing_pass()
                    .max_rect(ui.max_rect()),
            );
            (
                id.with(index),
                child
                    .add(button(name, index == excel.selected.get()))
                    .rect
                    .size(),
            )
        })
        .collect()
}
