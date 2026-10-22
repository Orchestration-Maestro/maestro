//! Cell painting for full and differential redraws.

use super::{cursor::write_position, style::build_sgr};
use crate::render::is_halfwidth_katakana_voiced_grapheme;
use maestro_protocol::frames::{CellData, FrameData};
use std::fmt::Write as _;
use unicode_width::UnicodeWidthStr;

/// Closes an OSC 8 hyperlink.
const LINK_END: &str = "\x1b]8;;\x1b\\";

/// Hyperlink targets with control characters removed; an empty result is no
/// link.
pub(super) fn sanitized_hyperlinks(frame: &FrameData) -> Vec<Option<String>> {
    frame
        .hyperlinks
        .iter()
        .map(|uri| {
            let sanitized: String = uri.chars().filter(|ch| !ch.is_control()).collect();
            (!sanitized.is_empty()).then_some(sanitized)
        })
        .collect()
}

/// A cell's sanitized link target, if its index names one.
fn cell_link<'a>(links: &'a [Option<String>], cell: &CellData) -> Option<&'a str> {
    links
        .get(usize::try_from(cell.hyperlink?).ok()?)?
        .as_deref()
}

/// Columns a cell covers on the host terminal.
fn cell_width(cell: &CellData) -> usize {
    if is_halfwidth_katakana_voiced_grapheme(&cell.symbol) {
        2
    } else {
        cell.symbol.width()
    }
}

/// Whether two cells look the same; the skip flag is not visual.
pub(super) fn cells_visually_equal(
    links: &[Option<String>],
    cell: &CellData,
    previous_links: &[Option<String>],
    previous: &CellData,
) -> bool {
    cell.symbol == previous.symbol
        && cell.fg == previous.fg
        && cell.bg == previous.bg
        && cell.modifier == previous.modifier
        && cell_link(links, cell) == cell_link(previous_links, previous)
}

/// A frame's cells by row, each with its row and column.
fn rows(frame: &FrameData) -> impl Iterator<Item = (u16, &[CellData])> {
    (0..frame.height).zip(frame.cells.chunks(usize::from(frame.width).max(1)))
}

/// Paints cells while tracking the active style, hyperlink and the cell the
/// host cursor reaches without a move.
struct CellWriter<'a> {
    /// Output being built.
    out: &'a mut String,
    /// Sanitized targets of the frame being written.
    links: &'a [Option<String>],
    /// Last SGR sequence written.
    last_sgr: String,
    /// Open hyperlink target.
    active_link: Option<&'a str>,
    /// Cell after the last written one when that cell advanced the cursor by
    /// exactly one column; hosts run without autowrap, so it stays in its row.
    next_inline: Option<(u16, u16)>,
}

impl<'a> CellWriter<'a> {
    /// Start writing a frame with no style or link active.
    fn new(out: &'a mut String, links: &'a [Option<String>]) -> Self {
        Self {
            out,
            links,
            last_sgr: String::new(),
            active_link: None,
            next_inline: None,
        }
    }

    /// Paint one cell, moving the cursor unless it is already there.
    fn write(&mut self, position: (u16, u16), cell: &CellData, reposition: bool) {
        if reposition || self.next_inline != Some(position) {
            write_position(self.out, position);
        }
        let sgr = build_sgr(cell.fg, cell.bg, cell.modifier);
        if sgr != self.last_sgr {
            self.out.push_str(&sgr);
            self.last_sgr = sgr;
        }
        let link = cell_link(self.links, cell);
        if link != self.active_link {
            if self.active_link.is_some() {
                self.out.push_str(LINK_END);
            }
            if let Some(uri) = link {
                let _ = write!(self.out, "\x1b]8;;{uri}\x1b\\");
            }
            self.active_link = link;
        }
        self.out.push_str(&cell.symbol);
        let (col, row) = position;
        self.next_inline = (cell.symbol.is_ascii() && cell_width(cell) == 1)
            .then_some((col.saturating_add(1), row));
    }

    /// Close an open hyperlink and report whether any cell was written.
    fn finish(self) -> bool {
        if self.active_link.is_some() {
            self.out.push_str(LINK_END);
        }
        !self.last_sgr.is_empty()
    }
}

/// Paint every visible cell, skipping cells covered by a wide grapheme.
pub(super) fn write_all_cells(out: &mut String, frame: &FrameData) {
    let links = sanitized_hyperlinks(frame);
    let mut writer = CellWriter::new(out, &links);
    for (row, cells) in rows(frame) {
        let mut to_skip = 0usize;
        for (col, cell) in (0..frame.width).zip(cells) {
            if to_skip > 0 {
                to_skip -= 1;
            } else if !cell.skip {
                writer.write((col, row), cell, false);
                to_skip = cell_width(cell).saturating_sub(1);
            }
        }
    }
    writer.finish();
    out.push_str("\x1b[0m");
}

/// Paint only the cells that differ from the previous same-size frame.
pub(super) fn write_changed_cells(out: &mut String, frame: &FrameData, previous: &FrameData) {
    let links = sanitized_hyperlinks(frame);
    let previous_links = sanitized_hyperlinks(previous);
    let mut writer = CellWriter::new(out, &links);
    let changes = Changes {
        frame,
        previous,
        links: &links,
        previous_links: &previous_links,
    };
    changes.for_each(|position, cell, revealed| writer.write(position, cell, revealed));
    if writer.finish() {
        out.push_str("\x1b[0m");
    }
}

/// Two same-size frames and their sanitized link targets.
pub(super) struct Changes<'a> {
    /// Frame to show.
    pub(super) frame: &'a FrameData,
    /// Frame the host shows now.
    pub(super) previous: &'a FrameData,
    /// Sanitized targets of the frame to show.
    pub(super) links: &'a [Option<String>],
    /// Sanitized targets of the shown frame.
    pub(super) previous_links: &'a [Option<String>],
}

impl Changes<'_> {
    /// Visit each cell a differential redraw paints, in row order, with
    /// whether a wider previous cell covered it.
    pub(super) fn for_each(&self, mut visit: impl FnMut((u16, u16), &CellData, bool)) {
        for ((row, cells), (_, previous_cells)) in rows(self.frame).zip(rows(self.previous)) {
            self.for_each_in_row(row, cells, previous_cells, &mut visit);
        }
    }

    /// Visit the changed cells of one row, revealing cells a previous wide
    /// grapheme covered and skipping cells a new one covers.
    fn for_each_in_row(
        &self,
        row: u16,
        cells: &[CellData],
        previous_cells: &[CellData],
        visit: &mut impl FnMut((u16, u16), &CellData, bool),
    ) {
        let (mut revealed, mut to_skip) = (0usize, 0usize);
        for (col, (cell, previous)) in (0..self.frame.width).zip(cells.iter().zip(previous_cells)) {
            let changed = revealed > 0
                || !cells_visually_equal(self.links, cell, self.previous_links, previous);
            if !cell.skip && to_skip == 0 && changed {
                visit((col, row), cell, revealed > 0);
            }
            to_skip = cell_width(cell).saturating_sub(1);
            revealed = cell_width(cell)
                .max(cell_width(previous))
                .max(revealed)
                .saturating_sub(1);
        }
    }
}
