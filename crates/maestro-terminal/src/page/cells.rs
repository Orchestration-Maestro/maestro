//! Cell edits and managed ownership release.

use super::{Cell, CellPosition, Page};
use std::ops::Range;

impl Page {
    /// Clear a selected range and release its page-owned suffixes and styles.
    pub fn clear_cells(&mut self, y: u16, columns: Range<u16>) {
        for x in columns.clone() {
            let position = CellPosition { x, y };
            self.clear_grapheme(position);
            self.clear_hyperlink(position);
            let style = self.get_cells(y)[usize::from(x)].style_id();
            if style != 0 {
                self.styles.release_multiple(style, 1);
            }
        }
        if columns.end - columns.start == self.size.cols {
            let has_placeholder = self
                .get_cells(y)
                .iter()
                .any(|cell| cell.codepoint() == '\u{10eeee}');
            let row = self.get_row_mut(y);
            row.set_grapheme(false);
            row.set_hyperlink(false);
            row.set_styled(false);
            if !has_placeholder {
                row.set_kitty_virtual_placeholder(false);
            }
        } else {
            self.update_row_grapheme_flag(y);
            self.update_row_styled_flag(y);
            self.update_row_hyperlink_flag(y);
        }
        self.get_cells_mut(y)[usize::from(columns.start)..usize::from(columns.end)]
            .fill(Cell::default());
    }

    /// Clear the styled flag when no addressable cell has a style ID.
    pub fn update_row_styled_flag(&mut self, y: u16) {
        if !self.get_cells(y).iter().any(|cell| cell.has_styling()) {
            self.get_row_mut(y).set_styled(false);
        }
    }
}

impl Page {
    /// Move selected cells, releasing overwritten content and zeroing the source.
    pub fn move_cells(&mut self, src: CellPosition, dst: CellPosition, len: u16) {
        self.clear_cells(dst.y, dst.x..dst.x + len);
        let managed = self.get_row(src.y).managed_memory();
        for offset in 0..len {
            let source = CellPosition {
                x: src.x + offset,
                y: src.y,
            };
            let destination = CellPosition {
                x: dst.x + offset,
                y: dst.y,
            };
            let cell = self.cells[self.cell_index(source)];
            let index = self.cell_index(destination);
            self.cells[index] = cell;
            if cell.has_grapheme() {
                self.move_grapheme(source, destination);
                self.get_row_mut(dst.y).set_grapheme(true);
            }
            if let Some(id) = self.hyperlink_map.remove(&self.cell_index(source)) {
                self.hyperlink_map.insert(index, id);
                self.get_row_mut(dst.y).set_hyperlink(true);
            }
            if managed && cell.codepoint() == '\u{10eeee}' {
                self.get_row_mut(dst.y).set_kitty_virtual_placeholder(true);
            }
            if cell.has_styling() {
                self.get_row_mut(dst.y).set_styled(true);
            }
        }
        self.get_cells_mut(src.y)[usize::from(src.x)..usize::from(src.x + len)]
            .fill(Cell::default());
        if len == self.size.cols {
            let row = self.get_row_mut(src.y);
            row.set_grapheme(false);
            row.set_hyperlink(false);
            row.set_styled(false);
            row.set_kitty_virtual_placeholder(false);
        }
    }
}

impl Page {
    /// Swap packed cell records and their suffix/link map entries without reallocating.
    pub fn swap_cells(&mut self, src: CellPosition, dst: CellPosition) {
        #[cfg(test)]
        {
            self.row_shift_work += 1;
        }
        let first = self.cell_index(src);
        let second = self.cell_index(dst);
        if first == second {
            return;
        }
        swap_entries(&mut self.graphemes, first, second);
        swap_entries(&mut self.hyperlink_map, first, second);
        self.cells.swap(first, second);
    }
}

/// Exchange only the two selected map entries, preserving absent sides.
fn swap_entries<T>(map: &mut std::collections::HashMap<usize, T>, first: usize, second: usize) {
    let left = map.remove(&first);
    let right = map.remove(&second);
    if let Some(value) = left {
        map.insert(second, value);
    }
    if let Some(value) = right {
        map.insert(first, value);
    }
}
