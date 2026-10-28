//! Read policy over the engine's retained rows and text formatters.

use super::{PaneCore, PaneTerminal, TerminalCursorState, retained::terminal_cell_text};
use crate::{render_prof, selection::Selection, terminal_theme::TerminalTheme};
use maestro_terminal::{
    Terminal,
    color::{Palette, Rgb},
    page::cell::Wide,
    render::RenderState,
    terminal::{TextRange, TextShape},
};
use std::time::Instant;

/// Retained text with an indication that physical rows were omitted.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TerminalReadSnapshot {
    /// Formatted selected rows.
    pub text: String,
    /// The row budget excluded older physical rows.
    pub truncated: bool,
}

impl TerminalReadSnapshot {
    /// Keeps the last `limit` lines of `text` with their line endings and reports whether
    /// older lines were left out; without a limit every line stays.
    #[must_use]
    pub fn limited(text: String, limit: Option<usize>) -> Self {
        let Some(limit) = limit else {
            return Self {
                text,
                truncated: false,
            };
        };
        let lines: Vec<&str> = text.split_inclusive('\n').collect();
        let omitted = lines.len().saturating_sub(limit);
        Self {
            text: lines[omitted..].concat(),
            truncated: omitted > 0,
        }
    }
}

/// One pane's refreshed render snapshot and the colour policy a frame reads,
/// borrowed under the pane lock.
pub struct PaneRenderView<'a> {
    /// Retained viewport rows, colours and dirty flags.
    pub render_state: &'a mut RenderState,
    /// Engine palette without the program's overrides.
    pub default_palette: Palette,
    /// Most recently observed host colours.
    pub host_terminal_theme: TerminalTheme,
    /// Engine default foreground when the pane was created.
    pub initial_default_foreground: Rgb,
    /// Engine default background when the pane was created.
    pub initial_default_background: Rgb,
    /// Whether images are drawn by the host instead of their placeholders.
    pub kitty_graphics_enabled: bool,
    /// Cursor presented at the supplied instant.
    pub cursor: Option<TerminalCursorState>,
    /// Profile start taken once the pane lock is held, when profiling is on.
    pub prof_started: Option<Instant>,
}

impl PaneTerminal {
    /// Refresh the single render snapshot and lend it with its colour policy
    /// under the pane lock; returning owned output keeps the borrows inside.
    pub fn with_render_view<T>(
        &self,
        now: Instant,
        read: impl FnOnce(PaneRenderView<'_>) -> T,
    ) -> Option<T> {
        let mut core = self.core.lock().ok()?;
        let prof_started = render_prof::timer();
        core.refresh_render_state();
        let cursor = core.reported_cursor_state(core.rendered_cursor_state(), now);
        let core = &mut *core;
        Some(read(PaneRenderView {
            default_palette: core.terminal.default_palette(),
            host_terminal_theme: core.host_terminal_theme,
            initial_default_foreground: core.initial_default_foreground,
            initial_default_background: core.initial_default_background,
            kitty_graphics_enabled: core.kitty_graphics_enabled,
            cursor,
            prof_started,
            render_state: &mut core.render_state,
        }))
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

/// Read each physical row cell by cell, preserving blank rows; a blank cell or
/// a whole image-placeholder grapheme reads as one space.
fn physical_text(t: &Terminal, start: usize, end: usize) -> String {
    let mut rows: Vec<String> = t
        .screen_text_rows_range(start, end)
        .unwrap_or_default()
        .into_iter()
        .map(|row| {
            let text: String = row
                .cells
                .iter()
                .filter(|cell| cell.wide != Wide::SpacerTail)
                .map(|cell| terminal_cell_text(&cell.graphemes))
                .collect();
            text.trim_end().to_owned()
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
