//! Locked retained search and expanding navigation windows.
use super::super::PaneTerminal;
use super::{
    Key, RetainedTextBuffer, ScreenTextRow, TerminalTextMatch, TerminalTextPoint,
    TerminalWordMotion,
};

impl PaneTerminal {
    /// Search all retained logical lines without crossing hard row boundaries.
    #[must_use]
    pub fn search_text_matches(&self, query: &str, case_sensitive: bool) -> Vec<TerminalTextMatch> {
        let Some((screen, cols, rows)) = self.screen_text_snapshot() else {
            return Vec::new();
        };
        RetainedTextBuffer::new_search(cols, rows, 0).search(query, case_sensitive, screen)
    }

    /// Revalidate a single retained search result.
    #[must_use]
    pub fn text_match_is_current(&self, text_match: TerminalTextMatch) -> bool {
        self.text_matches_are_current(&[text_match])
            .first()
            .copied()
            .unwrap_or(false)
    }

    /// Revalidate one bounded row window for a batch of search results.
    #[must_use]
    pub fn text_matches_are_current(&self, matches: &[TerminalTextMatch]) -> Vec<bool> {
        if matches.is_empty() {
            return Vec::new();
        }
        let Ok(core) = self.core.lock() else {
            return vec![false; matches.len()];
        };
        let cols = core.terminal.cols();
        let screen = core.terminal.active_screen();
        let range = matches
            .iter()
            .filter(|m| m.scan_cols == cols && m.scan_screen == screen)
            .fold(None::<(u32, u32)>, |range, m| {
                Some(range.map_or((m.start.row, m.end.row), |(a, b)| {
                    (a.min(m.start.row), b.max(m.end.row))
                }))
            });
        let Some((start, end)) = range else {
            return vec![false; matches.len()];
        };
        let (Ok(first), Ok(last)) = (
            usize::try_from(start),
            usize::try_from(end.saturating_add(1)),
        ) else {
            return vec![false; matches.len()];
        };
        let Some(rows) = core.terminal.screen_text_rows_range(first, last) else {
            return vec![false; matches.len()];
        };
        let buffer = RetainedTextBuffer::new_search(cols, rows, start);
        matches
            .iter()
            .map(|m| m.scan_cols == cols && m.scan_screen == screen && buffer.contains_match(*m))
            .collect()
    }

    /// Navigate from an absolute cell, expanding only when its word crosses the window.
    #[must_use]
    pub fn word_motion_target(
        &self,
        row: u32,
        col: u16,
        motion: TerminalWordMotion,
    ) -> Option<TerminalTextPoint> {
        let core = self.core.lock().ok()?;
        let cols = core.terminal.cols();
        let total = core.terminal.total_rows();
        let row = usize::try_from(row).ok()?;
        if row >= total {
            return None;
        }
        let mut window = 64usize;
        loop {
            let backward = matches!(
                motion,
                TerminalWordMotion::PreviousStart | TerminalWordMotion::PreviousBigStart
            );
            let (start, end) = if backward {
                (row.saturating_sub(window.saturating_sub(1)), row + 1)
            } else {
                (row, row.saturating_add(window).min(total))
            };
            let rows = core.terminal.screen_text_rows_range(start, end)?;
            let continued_start =
                start > 0 && rows.first().is_some_and(|row| row.wrap_continuation);
            let continued_end = end < total && rows.last().is_some_and(|row| row.soft_wrapped);
            let start_row = u32::try_from(start).ok()?;
            let buffer = RetainedTextBuffer::new_words(cols, rows, start_row);
            let target = buffer.word_motion(u32::try_from(row).ok()?, col, motion);
            let needs_history =
                backward && target.is_some_and(|p| continued_start && p.row == start_row);
            let needs_future = matches!(
                motion,
                TerminalWordMotion::NextEnd | TerminalWordMotion::NextBigEnd
            ) && continued_end
                && target.is_some_and(|p| buffer.point_is_final_atom(p));
            if target.is_some() && !needs_history && !needs_future
                || if backward { start == 0 } else { end == total }
            {
                return target;
            }
            window = window.saturating_mul(2).min(total);
        }
    }

    /// Copy one coherent screen identity, width and physical row snapshot.
    #[must_use]
    pub fn screen_text_snapshot(&self) -> Option<(Key, u16, Vec<ScreenTextRow>)> {
        let core = self.core.lock().ok()?;
        Some((
            core.terminal.active_screen(),
            core.terminal.cols(),
            core.terminal.screen_text_rows()?,
        ))
    }
}
