//! A pane exposes its child's final cursor to the host terminal.

use maestro_app::daemon::PaneTerminal;
use maestro_app::terminal::Terminal;
use maestro_multiplexer::render::pane::render;
use ratatui::{Terminal as Frames, backend::TestBackend, layout::Rect};

#[test]
fn daemon_agent_terminal_keeps_final_child_cursor_exposed() {
    let pane = PaneTerminal::new(Terminal::new(20, 5, 0).unwrap());
    pane.seed_history_ansi("agent> working\x1b[3;7H");
    let mut frames = Frames::new(TestBackend::new(30, 8)).unwrap();

    frames
        .draw(|frame| render(&pane, frame, Rect::new(4, 2, 20, 5), true))
        .unwrap();

    frames.backend_mut().assert_cursor_position((4 + 6, 2 + 2));
}
