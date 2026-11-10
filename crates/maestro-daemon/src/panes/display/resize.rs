//! Resize and scroll policy over the concrete terminal.

use super::{
    PaneTerminal, ScrollMetrics,
    reads::{ReadFormat, recent},
};
use maestro_terminal::{Terminal, screens::Key, terminal::ScrollViewport};

impl PaneTerminal {
    /// Resize while preserving the bottom-relative scroll position and return replies.
    #[must_use]
    pub fn resize(
        &self,
        rows: u16,
        cols: u16,
        cell_width_px: u32,
        cell_height_px: u32,
    ) -> Vec<Vec<u8>> {
        let Ok(mut core) = self.core.lock() else {
            return Vec::new();
        };
        let offset = metrics(&mut core.terminal).offset_from_bottom;
        let probe_lines = usize::from(rows).saturating_mul(8).max(24);
        let bottom = recent(
            &core.terminal,
            usize::from(core.terminal.rows()),
            ReadFormat::Physical,
        )
        .text;
        let replay = (core.terminal.active_screen() == Key::Primary && !bottom.trim().is_empty())
            .then(|| recent(&core.terminal, probe_lines, ReadFormat::UnwrappedAnsi).text);
        if core
            .terminal
            .resize(cols, rows, cell_width_px, cell_height_px)
            .is_err()
        {
            return Vec::new();
        }
        let mut replies = std::mem::take(&mut core.pending_pty_responses);
        let response = core.terminal.take_pty_reply();
        if !response.is_empty() {
            replies.push(response);
        }
        let blank = recent(&core.terminal, usize::from(rows), ReadFormat::Physical)
            .text
            .trim()
            .is_empty();
        if blank && let Some(ansi) = replay.filter(|text| !text.trim().is_empty()) {
            core.terminal.scroll_viewport(ScrollViewport::Bottom);
            core.terminal.write(ansi.as_bytes());
        }
        set_offset(&mut core.terminal, offset);
        let mut remaining = offset.min(probe_lines);
        while remaining > 0 && core.visible_text().trim().is_empty() {
            core.terminal.scroll_viewport(ScrollViewport::Delta(1));
            remaining -= 1;
        }
        replies
    }

    /// Move toward the oldest retained row.
    pub fn scroll_up(&self, lines: usize) {
        if let Ok(mut core) = self.core.lock() {
            core.terminal.scroll_viewport(ScrollViewport::Delta(
                -isize::try_from(lines).unwrap_or(isize::MAX),
            ));
        }
    }

    /// Move toward the live bottom row.
    pub fn scroll_down(&self, lines: usize) {
        if let Ok(mut core) = self.core.lock() {
            core.terminal.scroll_viewport(ScrollViewport::Delta(
                isize::try_from(lines).unwrap_or(isize::MAX),
            ));
        }
    }

    /// Resume following live output.
    pub fn scroll_reset(&self) {
        if let Ok(mut core) = self.core.lock() {
            core.terminal.scroll_viewport(ScrollViewport::Bottom);
        }
    }

    /// Set a clamped offset from the live bottom.
    pub fn set_scroll_offset_from_bottom(&self, lines: usize) {
        if let Ok(mut core) = self.core.lock() {
            set_offset(&mut core.terminal, lines);
        }
    }

    /// Observe the retained scroll interval.
    #[must_use]
    pub fn scroll_metrics(&self) -> Option<ScrollMetrics> {
        Some(metrics(&mut self.core.lock().ok()?.terminal))
    }
}

/// Read bottom-relative offsets from the engine's scrollbar.
fn metrics(t: &mut Terminal) -> ScrollMetrics {
    let bar = t.scrollbar();
    ScrollMetrics {
        offset_from_bottom: bar.total.saturating_sub(bar.offset.saturating_add(bar.len)),
        max_offset_from_bottom: bar.total.saturating_sub(bar.len),
        viewport_rows: bar.len,
    }
}

/// Select bottom explicitly so future output follows after history disappears.
fn set_offset(t: &mut Terminal, offset: usize) {
    let maximum = metrics(t).max_offset_from_bottom;
    let offset = offset.min(maximum);
    t.scroll_viewport(if offset == 0 {
        ScrollViewport::Bottom
    } else {
        ScrollViewport::Row(maximum - offset)
    });
}
