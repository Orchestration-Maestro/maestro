//! Primary-button pointer input delivered to canvas frames.

/// One pointer transition at a supplied canvas point.
#[must_use]
pub fn pointer(pos: egui::Pos2, pressed: bool) -> Vec<egui::Event> {
    vec![
        egui::Event::PointerMoved(pos),
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        },
    ]
}

/// A complete pointer activation, without a timer or input-policy substitute.
#[must_use]
pub fn click(pos: egui::Pos2) -> Vec<egui::Event> {
    let mut events = pointer(pos, true);
    events.push(egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    events
}
