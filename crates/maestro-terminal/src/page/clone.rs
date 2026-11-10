//! Selected-row copying with page-local managed ownership.

use super::{
    Cell, CellPosition, Page, StorageError,
    cell::{Content, Wide},
    row::Row,
};
use crate::{
    Style,
    links::{Hyperlink, Id},
};
use std::{borrow::Cow, ops::Range};

/// A style already resident in this page or copied from another page.
enum StyleCopy {
    /// Resident style identity.
    Local(u16),
    /// A foreign page's style value.
    Foreign(Style),
}
/// A link already resident in this page or borrowed from another page.
#[derive(Clone, Copy)]
enum LinkCopy<'a> {
    /// Resident hyperlink identity.
    Local(Id),
    /// A foreign page's hyperlink bytes.
    Foreign(&'a Hyperlink),
}
/// Selected cell data, borrowing foreign values or capturing a local suffix once.
struct CellCopy<'a> {
    /// Packed source record.
    cell: Cell,
    /// Suffix scalars, excluding the first scalar.
    suffix: Option<Cow<'a, [char]>>,
    /// Source style identity or value.
    style: Option<StyleCopy>,
    /// Source hyperlink identity or value.
    link: Option<LinkCopy<'a>>,
}

impl Page {
    /// Copy selected source rows to destination row zero, truncating columns.
    ///
    /// # Errors
    /// Returns the corresponding capacity error when the requested storage cannot fit.
    ///
    /// # Panics
    /// Panics when the source range or destination row count is out of bounds.
    pub fn clone_from(&mut self, other: &Self, rows: Range<u16>) -> Result<(), StorageError> {
        assert!(rows.end <= other.size.rows && rows.end - rows.start <= self.size.rows);
        for (dst, src) in (0..self.size.rows).zip(rows) {
            self.clone_row_from(other, dst, src)?;
        }
        Ok(())
    }

    /// Copy one whole row, truncating at either page's addressable column count.
    ///
    /// # Errors
    /// Returns the corresponding capacity error when the requested storage cannot fit.
    pub fn clone_row_from(
        &mut self,
        other: &Self,
        dst_row: u16,
        src_row: u16,
    ) -> Result<(), StorageError> {
        self.clone_partial_row_from(other, dst_row, src_row, 0..self.size.cols)
    }

    /// Copy a selected column range, preserving unselected destination cells.
    ///
    /// # Errors
    /// Returns the corresponding capacity error when the requested storage cannot fit.
    pub fn clone_partial_row_from(
        &mut self,
        other: &Self,
        dst_row: u16,
        src_row: u16,
        columns: Range<u16>,
    ) -> Result<(), StorageError> {
        let columns = columns.start..columns.end.min(self.size.cols).min(other.size.cols);
        self.clear_cells(dst_row, columns.clone());
        self.copy_row_metadata(
            *other.get_row(src_row),
            dst_row,
            columns.end - columns.start,
        );
        for x in columns.clone() {
            let src = CellPosition { x, y: src_row };
            self.clone_cell_from(other, CellPosition { x, y: dst_row }, src)?;
        }
        if self.size.cols > other.size.cols && columns.end == other.size.cols && !columns.is_empty()
        {
            let cell = &mut self.get_cells_mut(dst_row)[usize::from(columns.end - 1)];
            if cell.wide() == Wide::SpacerHead {
                cell.set_wide(Wide::Narrow);
            }
        }
        Ok(())
    }

    /// Copy one cell through the shared managed-value ownership path.
    pub(crate) fn clone_cell_from(
        &mut self,
        other: &Self,
        dst: CellPosition,
        src: CellPosition,
    ) -> Result<(), StorageError> {
        let cell = other.cells[other.cell_index(src)];
        let data = CellCopy {
            cell,
            suffix: other
                .lookup_grapheme(src)
                .filter(|_| cell.has_grapheme())
                .map(Cow::Borrowed),
            style: cell
                .has_styling()
                .then(|| StyleCopy::Foreign(*other.styles.get(cell.style_id()))),
            link: other
                .lookup_hyperlink(src)
                .filter(|_| cell.hyperlink())
                .map(|id| LinkCopy::Foreign(other.hyperlink_set.get(id))),
        };
        self.copy_cell(data, dst)
    }

    /// Copy selected data once before editing another row of the same page.
    ///
    /// # Errors
    /// Returns the corresponding capacity error when the requested storage cannot fit.
    pub fn clone_partial_row_within(
        &mut self,
        dst_row: u16,
        src_row: u16,
        columns: Range<u16>,
    ) -> Result<(), StorageError> {
        if dst_row == src_row {
            return Ok(());
        }
        let columns = columns.start..columns.end.min(self.size.cols);
        let source_row = *self.get_row(src_row);
        let captured: Vec<_> = columns
            .clone()
            .map(|x| {
                let src = CellPosition { x, y: src_row };
                let cell = self.cells[self.cell_index(src)];
                CellCopy {
                    cell,
                    suffix: self
                        .lookup_grapheme(src)
                        .map(|cps| Cow::Owned(cps.to_vec())),
                    style: cell
                        .has_styling()
                        .then_some(StyleCopy::Local(cell.style_id())),
                    link: self
                        .lookup_hyperlink(src)
                        .filter(|_| cell.hyperlink())
                        .map(LinkCopy::Local),
                }
            })
            .collect();
        self.clear_cells(dst_row, columns.clone());
        self.copy_row_metadata(source_row, dst_row, columns.end - columns.start);
        for (x, data) in columns.zip(captured) {
            self.copy_cell(data, CellPosition { x, y: dst_row })?;
        }
        Ok(())
    }

    /// Publish one cell's record and page-owned managed values in source order.
    fn copy_cell(&mut self, data: CellCopy<'_>, dst: CellPosition) -> Result<(), StorageError> {
        let index = self.cell_index(dst);
        self.cells[index] = data.cell;
        self.cells[index].set_style_id(0);
        self.cells[index].set_hyperlink(false);
        if let Some(cps) = data.suffix {
            self.cells[index].set_content(Content::Codepoint(data.cell.codepoint()));
            self.set_graphemes(dst, &cps)?;
        }
        if let Some(link) = data.link {
            let id = self.copy_link(link)?;
            self.set_hyperlink(dst, id)?;
        }
        if let Some(style) = data.style {
            self.get_row_mut(dst.y).set_styled(true);
            let id = match style {
                StyleCopy::Local(id) => {
                    self.styles.use_multiple(id, 1);
                    id
                }
                StyleCopy::Foreign(value) => self
                    .styles
                    .add(value)
                    .map_err(|_| StorageError::StyleSetOutOfMemory)?,
            };
            self.cells[index].set_style_id(id);
        }
        Ok(())
    }

    /// Retain a local link or copy a foreign link after map admission.
    fn copy_link(&mut self, link: LinkCopy<'_>) -> Result<Id, StorageError> {
        if self.hyperlink_count() >= self.hyperlink_capacity() {
            return Err(StorageError::HyperlinkMapOutOfMemory);
        }
        match link {
            LinkCopy::Local(id) => {
                self.hyperlink_set.use_multiple(id, 1);
                Ok(id)
            }
            LinkCopy::Foreign(value) => {
                self.insert_hyperlink(value.clone())
                    .map_err(|error| match error {
                        StorageError::StringsOutOfMemory => StorageError::StringAllocOutOfMemory,
                        _ => StorageError::HyperlinkSetOutOfMemory,
                    })
            }
        }
    }

    /// Copy row metadata while retaining flags belonging to unselected columns.
    fn copy_row_metadata(&mut self, mut source: Row, dst_row: u16, count: u16) {
        let previous = *self.get_row(dst_row);
        if count < self.size.cols {
            source.set_wrap(previous.wrap());
            source.set_wrap_continuation(previous.wrap_continuation());
            source.set_grapheme(previous.grapheme());
            source.set_hyperlink(previous.hyperlink());
            source.set_styled(previous.styled());
            source.set_kitty_virtual_placeholder(
                source.kitty_virtual_placeholder() || previous.kitty_virtual_placeholder(),
            );
            source.set_dirty(source.dirty() || previous.dirty());
        }
        self.rows[usize::from(dst_row)] = source;
    }
}

impl Page {
    /// Minimum logical budgets for selected rows, counting unique styles and links.
    #[must_use]
    ///
    /// # Panics
    /// Panics for an empty or out-of-bounds row range.
    pub fn exact_row_capacity(&self, rows: Range<u16>) -> super::layout::Capacity {
        use crate::bitmap_allocator::BitmapAllocator;
        use std::collections::HashSet;
        assert!(rows.start < rows.end && rows.end <= self.size.rows);
        let mut ids = HashSet::new();
        let mut grapheme_bytes = 0;
        for (position, cell) in self.cells_in_rows(rows.clone()) {
            if cell.has_styling() {
                ids.insert(cell.style_id());
            }
            if cell.has_grapheme()
                && let Some(cps) = self.lookup_grapheme(position)
            {
                grapheme_bytes += BitmapAllocator::<16>::bytes_required::<char>(cps.len());
            }
        }
        let styles = crate::style::Set::capacity_for_count(ids.len());
        ids.clear();
        let mut link_cells = 0_usize;
        let mut string_bytes = 0;
        for (position, cell) in self.cells_in_rows(rows.clone()) {
            if !cell.hyperlink() {
                continue;
            }
            link_cells += 1;
            if let Some(id) = self.lookup_hyperlink(position)
                && ids.insert(id)
            {
                string_bytes += self.hyperlink_set.get(id).string_bytes();
            }
        }
        let link_capacity =
            crate::links::Set::capacity_for_count(ids.len()).max(link_cells.div_ceil(16));
        super::layout::Capacity {
            cols: self.size.cols,
            rows: rows.end - rows.start,
            styles: u16::try_from(styles).unwrap_or(u16::MAX),
            hyperlink_bytes: u16::try_from(link_capacity * 48).unwrap_or(u16::MAX),
            grapheme_bytes: u32::try_from(grapheme_bytes).unwrap_or(u32::MAX),
            string_bytes: u32::try_from(string_bytes).unwrap_or(u32::MAX),
        }
    }
}

#[cfg(test)]
mod tests;

impl Page {
    /// Traverse each selected cell once, retaining its row and column identity.
    fn cells_in_rows(&self, rows: Range<u16>) -> impl Iterator<Item = (CellPosition, &Cell)> {
        rows.flat_map(move |y| {
            (0..self.size.cols)
                .zip(self.get_cells(y))
                .map(move |(x, cell)| (CellPosition { x, y }, cell))
        })
    }
}
