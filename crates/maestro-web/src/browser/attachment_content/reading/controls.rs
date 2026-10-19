//! Clipped two-endpoint primary-pointer admission for native reading controls.
/// Own both ends of primary gestures in canvas points, not native CSS pixels.
pub(crate) fn pointer_actions<A: Copy + PartialEq>(
    events: &[egui::Event],
    pressed: &mut Option<A>,
    visible_canvas_slots: &[(A, egui::Rect, egui::LayerId)],
    context: &egui::Context,
) -> Vec<A> {
    let mut actions = Vec::new();
    for event in events {
        if let egui::Event::PointerButton {
            pos: canvas_points,
            button: egui::PointerButton::Primary,
            pressed: down,
            ..
        } = event
        {
            let hit = visible_canvas_slots
                .iter()
                // Completed clicks use stale geometry; keep only toolkit layer admission.
                .find(|(_, rect, layer)| pointer_in_rect(context, *layer, *rect, *canvas_points))
                .map(|(action, _, _)| *action);
            if *down {
                *pressed = hit;
            } else if let Some(owner) = pressed.take()
                && hit == Some(owner)
            {
                actions.push(owner);
            }
        }
    }
    actions
}

/// Paint the focused control's border and reveal newly reached keyboard focus.
pub(crate) fn focus_border(
    ui: &egui::Ui,
    response: &egui::Response,
    presentation: &maestro_theme::BrandPresentation<'_>,
) -> Result<(), crate::tools::types::ToolUiError> {
    use crate::{browser::attachment_content, tools::types::ToolUiError};
    if response.gained_focus() {
        response.scroll_to_me(None);
    }
    if response.has_focus() {
        ui.painter().rect_stroke(
            response.rect,
            0.,
            egui::Stroke::new(
                1.,
                attachment_content::color(presentation, "accent")
                    .map_err(|error| ToolUiError::Presentation(error.to_string()))?,
            ),
            egui::StrokeKind::Inside,
        );
    }
    Ok(())
}

/// Keep event admission bounded while collecting unclipped paint for final follow placement.
#[cfg(target_arch = "wasm32")]
pub(crate) fn set_pointer_clip(
    context: &egui::Context,
    layer: egui::LayerId,
    clip: egui::Rect,
) -> egui::Id {
    let key = layer.id.with("reading-pointer-clip");
    context.data_mut(|data| data.insert_temp(key, clip));
    key
}
/// Project endpoints into the control's layer while retaining viewport and modal admission.
pub(crate) fn pointer_in_rect(
    context: &egui::Context,
    layer: egui::LayerId,
    rect: egui::Rect,
    point: egui::Pos2,
) -> bool {
    let local = context
        .layer_transform_from_global(layer)
        .map_or(point, |transform| transform * point);
    let clip =
        context.data(|data| data.get_temp::<egui::Rect>(layer.id.with("reading-pointer-clip")));
    let top = context.layer_id_at(point);
    let parent = context.memory(|memory| memory.areas().parent_layer(layer));
    rect.contains(local)
        && clip.is_none_or(|clip| clip.contains(point))
        && (top == Some(layer) || (parent.is_some() && top == parent))
}

/// Apply the same layer and viewport admission to hover decoration.
pub(crate) fn rect_contains_pointer(ui: &egui::Ui, rect: egui::Rect) -> bool {
    ui.input(|input| input.pointer.hover_pos())
        .is_some_and(|point| pointer_in_rect(ui.ctx(), ui.layer_id(), rect, point))
}
