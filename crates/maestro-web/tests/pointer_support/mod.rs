//! Complete pointer input shared by native and browser test programs.
#![cfg(test)]

/// A complete primary click at one position: move there, press, release.
#[must_use]
pub fn click(pos: egui::Pos2) -> Vec<egui::Event> {
    let button = |pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    vec![egui::Event::PointerMoved(pos), button(true), button(false)]
}
