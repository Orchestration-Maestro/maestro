//! A recording terminal whose output is replayed by a terminal emulator.
//!
//! The emulator is a development aid, not a reference: it applies the writer's bytes
//! with its own policies. It does not pull scrollback back into rows gained by a
//! height increase, and its screen clear differs from a cursor-line-preserving clear,
//! so scenarios compare the emitted bytes first and the screen only where the
//! emulator agrees with real terminals. It applies writes as they arrive, so an
//! observation never needs a flush.

use std::cell::RefCell;
use std::rc::Rc;

use maestro_tui::tui::TerminalHandle;

use super::recording_terminal::RecordingTerminal;

/// Scrollback rows the emulator keeps.
const SCROLLBACK_ROWS: usize = 1000;

/// Shared handle to an emulated terminal; clones observe the same screen.
#[derive(Clone)]
pub struct VirtualTerminal {
    /// Records the writer's effects and delivers input and resize notices.
    port: RecordingTerminal,
    /// Emulator that applies every recorded byte.
    parser: Rc<RefCell<vt100::Parser>>,
}

/// Converts a size to the emulator's `u16` rows and columns.
fn dimension(value: usize) -> u16 {
    u16::try_from(value).unwrap_or(u16::MAX)
}

impl VirtualTerminal {
    /// Creates a blank terminal of the given size.
    pub fn new(columns: usize, rows: usize) -> Self {
        let parser = Rc::new(RefCell::new(vt100::Parser::new(
            dimension(rows),
            dimension(columns),
            SCROLLBACK_ROWS,
        )));
        let port = RecordingTerminal::new(columns, rows);
        let sink = Rc::clone(&parser);
        port.set_tap(move |data| sink.borrow_mut().process(data.as_bytes()));
        Self { port, parser }
    }

    /// The terminal as the writer receives it.
    pub fn handle(&self) -> TerminalHandle {
        self.port.handle()
    }

    /// Writes and cursor visibility escapes the writer requested, in order.
    pub fn writes(&self) -> Vec<String> {
        self.port.writes()
    }

    /// Delivers input to the started writer.
    pub fn send_input(&self, data: &str) {
        self.port.send_input(data);
    }

    /// Resizes the emulated screen and notifies the started writer.
    pub fn resize(&self, columns: usize, rows: usize) {
        self.parser
            .borrow_mut()
            .screen_mut()
            .set_size(dimension(rows), dimension(columns));
        self.port.resize(columns, rows);
    }

    /// The visible rows without trailing blanks.
    pub fn viewport(&self) -> Vec<String> {
        let parser = self.parser.borrow();
        let screen = parser.screen();
        let (_, columns) = screen.size();
        screen
            .rows(0, columns)
            .map(|row| row.trim_end().to_owned())
            .collect()
    }

    /// Every retained row, oldest first: the scrollback followed by the visible rows.
    pub fn history(&self) -> Vec<String> {
        let mut parser = self.parser.borrow_mut();
        let screen = parser.screen_mut();
        let (_, columns) = screen.size();
        screen.set_scrollback(usize::MAX);
        let retained = screen.scrollback();
        let mut lines = Vec::new();
        for offset in (0..=retained).rev() {
            screen.set_scrollback(offset);
            lines.extend(screen.rows(0, columns).next());
        }
        lines.extend(screen.rows(0, columns).skip(1));
        lines
            .iter()
            .map(|line| line.trim_end().to_owned())
            .collect()
    }

    /// The cursor as `(column, row)`; a pending wrap leaves the column at the width.
    pub fn cursor(&self) -> (usize, usize) {
        let (row, column) = self.parser.borrow().screen().cursor_position();
        (usize::from(column), usize::from(row))
    }

    /// Whether the visible cell is italic.
    pub fn is_italic(&self, row: usize, column: usize) -> bool {
        let parser = self.parser.borrow();
        let cell = parser.screen().cell(dimension(row), dimension(column));
        cell.is_some_and(vt100::Cell::italic)
    }
    /// Whether the visible cell is underlined.
    pub fn is_underlined(&self, row: usize, column: usize) -> bool {
        let parser = self.parser.borrow();
        let cell = parser.screen().cell(dimension(row), dimension(column));
        cell.is_some_and(vt100::Cell::underline)
    }
}
