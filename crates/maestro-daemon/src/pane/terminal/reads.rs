//! Read policy over the engine's retained rows and text formatters.

use super::{PaneCore, PaneTerminal};
use crate::selection::Selection;
use maestro_terminal::{
    Terminal,
    terminal::{TextRange, TextShape},
};

/// Retained text with an indication that physical rows were omitted.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TerminalReadSnapshot {
    /// Formatted selected rows.
    pub text: String,
    /// The row budget excluded older physical rows.
    pub truncated: bool,
}

impl PaneTerminal {
    /// Access the single refreshed render snapshot under the pane lock.
    /// Returning owned output keeps snapshot borrows inside the lock.
    pub fn with_render_state<T>(
        &self,
        read: impl FnOnce(&mut maestro_terminal::render::RenderState) -> T,
    ) -> Option<T> {
        let mut core = self.core.lock().ok()?;
        core.refresh_render_state();
        Some(read(&mut core.render_state))
    }

    /// Visible physical rows, omitting trailing blank rows.
    #[must_use]
    pub fn visible_text(&self) -> String {
        self.core
            .lock()
            .map(|mut core| core.visible_text())
            .unwrap_or_default()
    }

    /// Styled visible physical rows.
    #[must_use]
    pub fn visible_ansi(&self) -> String {
        self.core
            .lock()
            .ok()
            .and_then(|core| {
                let t = &core.terminal;
                t.read_ansi_viewport(full_range(t, 0, usize::from(t.rows()))?)
            })
            .unwrap_or_default()
    }

    /// Live bottom rows, independent of the scrolled viewport.
    #[must_use]
    pub fn detection_text(&self) -> String {
        self.core
            .lock()
            .map(|core| {
                recent(
                    &core.terminal,
                    usize::from(core.terminal.rows()),
                    ReadFormat::Physical,
                )
                .text
            })
            .unwrap_or_default()
    }

    /// Recent physical rows with omitted-row reporting.
    #[must_use]
    pub fn recent_text_snapshot(&self, lines: usize) -> TerminalReadSnapshot {
        self.read_recent(lines, ReadFormat::Physical)
    }

    /// Recent logical text with soft wraps joined.
    #[must_use]
    pub fn recent_unwrapped_text_snapshot(&self, lines: usize) -> TerminalReadSnapshot {
        self.read_recent(lines, ReadFormat::Unwrapped)
    }

    /// Recent styled physical rows.
    #[must_use]
    pub fn recent_ansi_snapshot(&self, lines: usize) -> TerminalReadSnapshot {
        self.read_recent(lines, ReadFormat::Ansi)
    }

    /// Recent styled logical rows with soft wraps joined.
    #[must_use]
    pub fn recent_unwrapped_ansi_snapshot(&self, lines: usize) -> TerminalReadSnapshot {
        self.read_recent(lines, ReadFormat::UnwrappedAnsi)
    }

    /// Recent replayable styled text without row-limit metadata.
    #[must_use]
    pub fn recent_unwrapped_ansi(&self, lines: usize) -> String {
        self.recent_unwrapped_ansi_snapshot(lines).text
    }

    /// Read inclusive absolute selection coordinates, independent of viewport.
    #[must_use]
    pub fn extract_selection(&self, selection: &Selection) -> Option<String> {
        let ((start_row, start_col), (end_row, end_col)) = selection.ordered_cells();
        self.core.lock().ok()?.terminal.read_text_screen(TextRange {
            start: (start_col, start_row),
            end: (end_col, end_row),
            shape: TextShape::Linear,
        })
    }

    /// Apply one recent-read policy while holding the screen lock.
    fn read_recent(&self, lines: usize, format: ReadFormat) -> TerminalReadSnapshot {
        self.core
            .lock()
            .map(|core| recent(&core.terminal, lines, format))
            .unwrap_or_default()
    }
}

impl PaneCore {
    /// Keep cursor, text and rendering on the same dirty consumer.
    pub(super) fn refresh_render_state(&mut self) {
        self.render_state.update(&mut self.terminal);
    }

    /// Read the current viewport with physical-row boundaries.
    pub(super) fn visible_text(&mut self) -> String {
        self.refresh_render_state();
        let scrollbar = self.terminal.scrollbar();
        physical_text(
            &self.terminal,
            scrollbar.offset,
            scrollbar.offset + scrollbar.len,
        )
    }
}

/// Recent row emission policy.
#[derive(Clone, Copy)]
pub(super) enum ReadFormat {
    /// Plain physical rows with a final newline.
    Physical,
    /// Plain logical rows.
    Unwrapped,
    /// Styled physical rows.
    Ansi,
    /// Styled logical rows.
    UnwrappedAnsi,
}

/// Clamp a half-open physical row interval into inclusive engine coordinates.
pub(super) fn full_range(t: &Terminal, start: usize, end: usize) -> Option<TextRange> {
    (start < end).then_some(TextRange {
        start: (0, u32::try_from(start).ok()?),
        end: (
            t.cols().saturating_sub(1),
            u32::try_from(end.checked_sub(1)?).ok()?,
        ),
        shape: TextShape::Linear,
    })
}

/// Read each physical row through the existing formatter, preserving blank rows.
fn physical_text(t: &Terminal, start: usize, end: usize) -> String {
    let mut rows: Vec<String> = (start..end)
        .map(|row| {
            full_range(t, row, row.saturating_add(1))
                .and_then(|range| t.read_text_screen(range))
                .unwrap_or_default()
                .replace('\u{10eeee}', " ")
                .trim_end()
                .to_owned()
        })
        .collect();
    trim_trailing_blank_rows(&mut rows);
    let mut text = rows.join("\n");
    if !text.is_empty() {
        text.push('\n');
    }
    text
}

/// Omit empty viewport tail rows without changing authored internal spacing.
pub(super) fn trim_trailing_blank_rows(rows: &mut Vec<String>) {
    while rows.last().is_some_and(|row| row.trim().is_empty()) {
        rows.pop();
    }
}

/// Emit the last physical rows; logical joining never changes the row budget.
pub(super) fn recent(t: &Terminal, lines: usize, format: ReadFormat) -> TerminalReadSnapshot {
    let total = t.total_rows();
    let start = total.saturating_sub(lines);
    let text = full_range(t, start, total)
        .and_then(|range| match format {
            ReadFormat::Physical => Some(physical_text(t, start, total)),
            ReadFormat::Unwrapped => t.read_text_screen(range),
            ReadFormat::Ansi => t.read_ansi_screen(range, false),
            ReadFormat::UnwrappedAnsi => t.read_ansi_screen(range, true),
        })
        .unwrap_or_default();
    TerminalReadSnapshot {
        text,
        truncated: total > lines,
    }
}
