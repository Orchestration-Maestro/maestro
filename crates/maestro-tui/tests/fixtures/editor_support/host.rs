#![cfg(test)]
//! Shared controlled editor construction.
use crate::input_support;
pub use maestro_tui::test_support::manual_runtime;
pub use maestro_tui::test_support::recording_terminal;
use maestro_tui::{EditorTheme, SelectListTheme, TUI, TerminalImage};
use std::rc::Rc;

pub use input_support::globals;
pub fn host(
    rows: usize,
) -> (
    TUI,
    recording_terminal::RecordingTerminal,
    manual_runtime::ManualRuntime,
) {
    let terminal = recording_terminal::RecordingTerminal::new(80, rows);
    let runtime = manual_runtime::ManualRuntime::new();
    let tui = TUI::new(
        terminal.handle(),
        runtime.handle(),
        TerminalImage::new(|_| None, || 1),
        None,
    );
    (tui, terminal, runtime)
}
pub fn theme() -> EditorTheme {
    let identity: Rc<dyn Fn(&str) -> String> = Rc::new(str::to_owned);
    EditorTheme {
        border_color: identity.clone(),
        select_list: SelectListTheme {
            selected_prefix: identity.clone(),
            selected_text: identity.clone(),
            description: identity.clone(),
            scroll_info: identity.clone(),
            no_match: identity,
        },
    }
}
