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
                .find(|(_, rect, layer)| {
                    rect.contains(*canvas_points)
                        && context.layer_id_at(*canvas_points) == Some(*layer)
                })
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
