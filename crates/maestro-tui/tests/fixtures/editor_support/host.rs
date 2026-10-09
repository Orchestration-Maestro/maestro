#![cfg(test)]
//! Shared controlled editor construction.
#[allow(dead_code, reason = "Only the shared global guard is used here.")]
#[path = "../input_support/mod.rs"]
mod input_support;
#[allow(
    dead_code,
    reason = "Shared controlled host supports other terminal tests."
)]
#[path = "../../../../maestro-test-terminal/tests/support/manual_runtime.rs"]
pub mod manual_runtime;
#[allow(
    dead_code,
    reason = "Shared controlled terminal supports other terminal tests."
)]
#[path = "../../../../maestro-test-terminal/tests/support/recording_terminal.rs"]
pub mod recording_terminal;
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
