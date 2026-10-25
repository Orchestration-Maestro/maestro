//! The real writer the loader tests drive.

use maestro_tui::{TUI, TerminalImage};

use super::manual_runtime::ManualRuntime;
use super::recording_terminal::RecordingTerminal;

/// A 16-column, 24-row writer with controlled time and recorded output.
pub fn writer() -> (TUI, RecordingTerminal, ManualRuntime) {
    let runtime = ManualRuntime::new();
    let terminal = RecordingTerminal::new(16, 24);
    let tui = TUI::new(
        terminal.handle(),
        runtime.handle(),
        TerminalImage::default(),
        None,
    );
    (tui, terminal, runtime)
}
