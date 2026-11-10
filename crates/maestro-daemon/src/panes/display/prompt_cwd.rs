//! Windows shell prompt cwd observations over retained text.
use super::{PaneCore, PaneTerminal, reads::full_range};
use maestro_terminal::{Coordinate, Point, page_list::Direction, screens::Key};
use std::path::PathBuf;

impl PaneTerminal {
    /// Enable default PowerShell prompt cwd observations for a known shell.
    pub fn set_windows_powershell_prompt_cwd_reporting(&self, enabled: bool) {
        if let Ok(mut core) = self.core.lock() {
            core.windows_powershell_prompt_cwd_reporting = enabled.into();
        }
    }
}

impl PaneCore {
    /// Read only the current primary-screen prompt, never a preceding output line.
    pub(super) fn windows_powershell_current_prompt_cwd(&mut self) -> Option<PathBuf> {
        if !bool::from(self.windows_powershell_prompt_cwd_reporting)
            || self.terminal.active_screen() != Key::Primary
        {
            return None;
        }
        let cursor = self.current_cursor_state()?;
        let rows = self.terminal.rows();
        if cursor.y >= rows {
            return None;
        }
        let start = self.terminal.total_rows().saturating_sub(usize::from(rows));
        let row = start + usize::from(cursor.y);
        let current = self
            .terminal
            .read_text_screen(full_range(&self.terminal, row, row + 1)?)?;
        if current.trim().is_empty() {
            return None;
        }
        let pages = &self.terminal.screens.active().pages;
        let anchor = pages.pin(Point::Active(Coordinate {
            x: 0,
            y: u32::from(cursor.y),
        }))?;
        let mut preceding = anchor.row_iterator(Direction::LeftUp, None);
        let mut logical_start = row;
        while let Some(pin) = preceding.next(pages) {
            if !pin.row_and_cell(pages)?.0.wrap_continuation() {
                break;
            }
            logical_start = logical_start.checked_sub(1)?;
        }
        let text =
            self.terminal
                .read_text_screen(full_range(&self.terminal, logical_start, row + 1)?)?;
        windows_powershell_prompt_cwd(&text)
    }
}

/// Use the last nonempty logical line as the current prompt candidate.
pub(super) fn windows_powershell_prompt_cwd(text: &str) -> Option<PathBuf> {
    text.lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .and_then(windows_powershell_prompt_line_cwd)
}

/// Accept only a default prompt ending at an existing absolute filesystem directory.
fn windows_powershell_prompt_line_cwd(line: &str) -> Option<PathBuf> {
    let rest = line.trim_end().strip_prefix("PS ")?;
    let marker = rest.find('>')?;
    let mut raw = rest[..marker].trim_end();
    if !rest[marker..].trim_end().chars().all(|ch| ch == '>') {
        return None;
    }
    if let Some((_, path)) = raw.rsplit_once("::") {
        raw = path;
    }
    let cwd = PathBuf::from(raw);
    (cwd.is_absolute() && cwd.is_dir()).then_some(cwd)
}
