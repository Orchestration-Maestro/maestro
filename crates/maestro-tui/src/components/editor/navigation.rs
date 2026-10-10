//! Original-byte visual positions with retained cell-column intent.
use super::{
    Editing, Editor, LastAction, markers,
    wrapping::{atoms, word_wrap_line},
};
use crate::autocomplete::CursorPosition;
use std::ops::Range;
/// One wrapped row in its logical line's byte and cell coordinates.
struct VisualRow {
    /// Owning logical line.
    line: usize,
    /// Original byte interval.
    bytes: Range<usize>,
    /// Absolute visible cell interval.
    cells: Range<usize>,
    /// Whether the logical endpoint belongs to this row.
    final_row: bool,
}
/// Column chosen for a vertical move: a cell number, or the destination line end.
enum Column {
    /// Exact atom boundary at this cell offset, or the start of the wide atom containing it.
    Cell(usize),
    /// End of the destination line, after any trailing zero-cell atoms.
    End,
}
impl VisualRow {
    /// Largest admitted local cell position.
    fn maximum(&self) -> usize {
        let width = self.cells.end - self.cells.start;
        if self.final_row {
            width
        } else {
            width.saturating_sub(1)
        }
    }
}
impl Editor {
    /// Chooses history only at its admitted empty or visual-edge branches.
    pub(super) fn vertical_input(&self, down: bool) {
        let history = {
            let state = self.state.borrow();
            let rows = state.visual_rows(self.width.get());
            let index = state.visual_index(&rows);
            let empty = state.current.lines.len() == 1 && state.current.lines[0].is_empty();
            if down {
                state.history_index.is_some() && index + 1 == rows.len()
            } else {
                empty || (state.history_index.is_some() && index == 0)
            }
        };
        if history {
            self.browse_history(down);
        } else {
            self.state.borrow_mut().vertical(
                self.width.get(),
                if down {
                    Direction::Forward
                } else {
                    Direction::Backward
                },
                1,
                false,
            );
        }
    }
}
impl Editing {
    /// Reuses drawing's original wrapping and visible atoms for visual identity.
    fn visual_rows(&self, width: usize) -> Vec<VisualRow> {
        let mut rows = Vec::new();
        for (line, text) in self.current.lines.iter().enumerate() {
            let atoms = atoms(text);
            let mut remaining = atoms.as_slice();
            let segments = markers::segments(text, self.pastes.len());
            let chunks = word_wrap_line(text, width, Some(&segments));
            let last = chunks.len() - 1;
            let mut cell = 0;
            for (index, chunk) in chunks.into_iter().enumerate() {
                let start = cell;
                let boundary = remaining.partition_point(|atom| atom.start < chunk.end_index);
                let (selected, rest) = remaining.split_at(boundary);
                remaining = rest;
                cell += selected.iter().map(|atom| atom.cells).sum::<usize>();
                rows.push(VisualRow {
                    line,
                    bytes: chunk.start_index..chunk.end_index,
                    cells: start..cell,
                    final_row: index == last,
                });
            }
        }
        rows
    }
    /// Locates the byte cursor; nonfinal endpoints belong to the following row.
    fn visual_index(&self, rows: &[VisualRow]) -> usize {
        let cursor = self.current.cursor;
        rows.iter()
            .position(|row| {
                row.line == cursor.line
                    && cursor.col >= row.bytes.start
                    && (cursor.col < row.bytes.end || row.final_row)
            })
            .unwrap_or(rows.len() - 1)
    }
    /// Retains the current visible column after Right at the document endpoint.
    pub(super) fn remember_visual_column(&mut self, width: usize) {
        let rows = self.visual_rows(width);
        self.preferred = Some(
            self.cursor_cell()
                .saturating_sub(rows[self.visual_index(&rows)].cells.start),
        );
    }
    /// Resolves a stored byte position to its visible atom's absolute cell start.
    fn cursor_cell(&self) -> usize {
        let cursor = self.current.cursor;
        atoms(&self.current.lines[cursor.line])
            .iter()
            .take_while(|atom| atom.end <= cursor.col)
            .map(|atom| atom.cells)
            .sum()
    }
    /// Moves to a visual neighbor or page target without normalizing stored text.
    pub(super) fn vertical(
        &mut self,
        width: usize,
        direction: Direction,
        count: usize,
        page: bool,
    ) {
        self.reset_action();
        let down = matches!(direction, Direction::Forward);
        let rows = self.visual_rows(width);
        let source = self.visual_index(&rows);
        let target = if down {
            (source + count).min(rows.len() - 1)
        } else {
            source.saturating_sub(count)
        };
        if source == target && !page {
            let col = if down {
                self.current.lines[self.current.cursor.line].len()
            } else {
                0
            };
            self.set_col(col);
            return;
        }
        self.move_to_row(&rows, source, target);
    }
    /// Applies the sticky-column decision before snapping a target to its visible atom.
    fn move_to_row(&mut self, rows: &[VisualRow], source: usize, target: usize) {
        let row = &rows[source];
        let absolute = self.snapped.unwrap_or_else(|| self.cursor_cell());
        let resolved = rows
            .iter()
            .find(|candidate| {
                candidate.line == row.line
                    && absolute >= candidate.cells.start
                    && (absolute < candidate.cells.end || candidate.final_row)
            })
            .unwrap_or(row);
        let current = absolute.saturating_sub(resolved.cells.start);
        let destination = &rows[target];
        let column = self.vertical_column(current, row.maximum(), destination);
        let (col, snapped) = match column {
            Column::Cell(column) => {
                match self.land(rows, (source, target), destination.cells.start + column) {
                    Ok(landed) => landed,
                    Err(next) => return self.move_to_row(rows, source, next),
                }
            }
            Column::End => (destination.bytes.end, None),
        };
        self.snapped = snapped;
        self.current.cursor = CursorPosition {
            line: destination.line,
            col,
        };
    }
    /// Chooses the byte column and remembered cell for an absolute cell of the target row.
    ///
    /// Moving down onto a row that continues a marker, the first row past that marker is returned
    /// as the error when one exists. Otherwise the cursor lands on the start of the unit under the
    /// target cell, or on the row's endpoint when no unit covers that cell.
    fn land(
        &self,
        rows: &[VisualRow],
        (source, target): (usize, usize),
        absolute: usize,
    ) -> Result<(usize, Option<usize>), usize> {
        let destination = &rows[target];
        let Some((start, end, cells)) = self.unit_at(destination, absolute) else {
            return Ok((destination.bytes.end, None));
        };
        if start < destination.bytes.start
            && target > source
            && let Some(next) = past_continuation(rows, target, end)
        {
            return Err(next);
        }
        Ok((start, (absolute > cells).then_some(absolute)))
    }
    /// Finds the edit unit of the destination row that holds an absolute cell.
    ///
    /// Returns its byte range start, byte range end and first absolute cell. An owned marker
    /// is one unit even where wrapping split it across rows.
    fn unit_at(&self, row: &VisualRow, absolute: usize) -> Option<(usize, usize, usize)> {
        let mut cells = 0;
        for unit in markers::units(&self.current.lines[row.line], self.pastes.len()) {
            let start = cells;
            cells += unit.cells;
            if unit.end > row.bytes.start
                && unit.start < row.bytes.end
                && (absolute == start || absolute < cells)
            {
                return Some((unit.start, unit.end, start));
            }
        }
        None
    }
    /// Restores preferred columns only from a clamped source row.
    ///
    /// A column clamped on the destination's final row is the line end, which
    /// keeps trailing zero-cell atoms before it; any other column is a cell
    /// number, and a clamp on a non-final row stays the row's last cell.
    fn vertical_column(
        &mut self,
        current: usize,
        source_maximum: usize,
        destination: &VisualRow,
    ) -> Column {
        let target_maximum = destination.maximum();
        let clamp = if destination.final_row {
            Column::End
        } else {
            Column::Cell(target_maximum)
        };
        let Some(preferred) = self.preferred.filter(|_| current >= source_maximum) else {
            self.preferred = (target_maximum < current).then_some(current);
            return if target_maximum < current {
                clamp
            } else {
                Column::Cell(current)
            };
        };
        if target_maximum < current || target_maximum < preferred {
            return clamp;
        }
        self.preferred = None;
        Column::Cell(preferred)
    }
    /// Changes the byte column and invalidates visual-column intent.
    pub(super) fn set_col(&mut self, col: usize) {
        self.revision = self.revision.wrapping_add(1);
        self.current.cursor.col = col;
        self.preferred = None;
        self.snapped = None;
    }
}
/// First row after `target` that starts at or beyond `end` or on another line.
fn past_continuation(rows: &[VisualRow], target: usize, end: usize) -> Option<usize> {
    let line = rows[target].line;
    let next = target
        + 1
        + rows[target + 1..]
            .iter()
            .take_while(|row| row.line == line && row.bytes.start < end)
            .count();
    (next < rows.len()).then_some(next)
}
impl Editing {
    /// Moves across one whitespace prefix and one owned marker, punctuation run or word run.
    pub(super) fn word(&mut self, forward: bool) {
        self.reset_action();
        let cursor = self.current.cursor;
        let text = &self.current.lines[cursor.line];
        if forward && cursor.col == text.len() {
            if cursor.line + 1 < self.current.lines.len() {
                self.current.cursor.line += 1;
                self.set_col(0);
            }
        } else if !forward && cursor.col == 0 {
            if cursor.line > 0 {
                self.current.cursor.line -= 1;
                self.set_col(self.current.lines[cursor.line - 1].len());
            }
        } else {
            let length = if forward {
                word_length(
                    markers::segments(&text[cursor.col..], self.pastes.len())
                        .into_iter()
                        .map(|(_, text)| text),
                )
            } else {
                let parts = markers::segments(&text[..cursor.col], self.pastes.len());
                word_length(parts.into_iter().rev().map(|(_, text)| text))
            };
            self.set_col(if forward {
                cursor.col + length
            } else {
                cursor.col - length
            });
        }
    }
}
/// Consumes leading whitespace, then one owned marker or one homogeneous punctuation or word run.
fn word_length<'a>(parts: impl Iterator<Item = &'a str>) -> usize {
    let mut parts = parts.peekable();
    let mut length = 0;
    while let Some(text) =
        parts.next_if(|text| !markers::is_marker(text) && crate::is_whitespace_char(text))
    {
        length += text.len();
    }
    let Some(first) = parts.peek() else {
        return length;
    };
    if markers::is_marker(first) {
        return length + first.len();
    }
    let punctuation = crate::is_punctuation_char(first);
    length
        + parts
            .take_while(|text| {
                !markers::is_marker(text)
                    && !crate::is_whitespace_char(text)
                    && crate::is_punctuation_char(text) == punctuation
            })
            .map(str::len)
            .sum::<usize>()
}

impl Editing {
    /// Invalidates pending action completion without changing visual intent.
    pub(super) fn reset_action(&mut self) {
        self.action = LastAction::None;
        self.revision = self.revision.wrapping_add(1);
    }
}
/// Literal search direction.
#[derive(Clone, Copy)]
pub(super) enum Direction {
    /// Later scalar starts and logical lines.
    Forward,
    /// Earlier scalar starts and logical lines.
    Backward,
}
impl Editor {
    /// Consumes pending printable searches; other controls cancel and fall through.
    pub(super) fn pending_jump(&self, data: &str, bindings: &crate::KeybindingsManager) -> bool {
        let Some(direction) = self.state.borrow_mut().jump.take() else {
            return false;
        };
        if bindings.matches(data, "tui.editor.jumpForward")
            || bindings.matches(data, "tui.editor.jumpBackward")
        {
            return true;
        }
        if let Some(scalar) = crate::keys::decode_printable_key(data) {
            self.state
                .borrow_mut()
                .jump_to(scalar.encode_utf8(&mut [0; 4]), direction);
            return true;
        }
        if data.chars().next().is_some_and(|scalar| scalar >= ' ') {
            self.state.borrow_mut().jump_to(data, direction);
            return true;
        }
        false
    }
}
impl Editing {
    /// Searches literal chunks at scalar starts, excluding the current byte position.
    fn jump_to(&mut self, text: &str, direction: Direction) {
        self.reset_action();
        let cursor = self.current.cursor;
        let forward = matches!(direction, Direction::Forward);
        let indices: Box<dyn Iterator<Item = usize>> = if forward {
            Box::new(cursor.line..self.current.lines.len())
        } else {
            Box::new((0..=cursor.line).rev())
        };
        for line in indices {
            let value = &self.current.lines[line];
            let region = if line != cursor.line {
                0..value.len()
            } else if forward {
                let after = value[cursor.col..]
                    .chars()
                    .next()
                    .map_or(cursor.col, |scalar| cursor.col + scalar.len_utf8());
                after..value.len()
            } else {
                0..cursor.col
            };
            let mut starts = value[region.clone()]
                .char_indices()
                .map(|(at, _)| region.start + at)
                .filter(|&at| value[at..].starts_with(text));
            let found = if forward {
                starts.next()
            } else {
                starts.next_back()
            };
            if let Some(col) = found {
                self.current.cursor.line = line;
                self.set_col(col);
                return;
            }
        }
    }
}
