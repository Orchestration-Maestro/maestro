//! Owned projections of terminal cells for behavioral assertions.

use maestro_terminal::{
    Coordinate, Point, Terminal,
    page::{Cell, CellPosition, cell::Wide},
};

pub(crate) fn cell(t: &Terminal, x: u16, y: u32) -> Cell {
    let screen = t.screens.active();
    let pin = screen
        .pages
        .pin(Point::Active(Coordinate { x, y }))
        .unwrap();
    *pin.row_and_cell(&screen.pages).unwrap().1
}

pub(crate) fn dirty(t: &Terminal, y: u32) -> bool {
    let screen = t.screens.active();
    let pin = screen
        .pages
        .pin(Point::Active(Coordinate { x: 0, y }))
        .unwrap();
    pin.is_dirty(&screen.pages).unwrap()
}

pub(crate) fn clear_dirty(t: &mut Terminal) {
    let screen = t.screens.active_mut();
    for y in 0..u32::from(screen.pages.rows()) {
        let pin = screen
            .pages
            .pin(Point::Active(Coordinate { x: 0, y }))
            .unwrap();
        pin.row_and_cell_mut(&mut screen.pages)
            .unwrap()
            .0
            .set_dirty(false);
    }
}

pub(crate) fn assert_text(t: &Terminal, expected: &str) {
    let screen = t.screens.active();
    let mut lines = expected.split('\n');
    for y in 0..u32::from(t.rows()) {
        let mut chars = lines.next().unwrap_or_default().chars();
        let pin = screen
            .pages
            .pin(Point::Active(Coordinate { x: 0, y }))
            .unwrap();
        let page = screen.pages.page(pin.node).unwrap();
        for (x, cell) in page.get_cells(pin.y).iter().enumerate() {
            if matches!(cell.wide(), Wide::SpacerTail | Wide::SpacerHead) {
                continue;
            }
            let cp = cell.codepoint();
            if let Some(expected) = chars.next() {
                assert_eq!(if cp == '\0' { ' ' } else { cp }, expected, "cell {x},{y}");
            } else {
                assert!(matches!(cp, '\0' | ' '), "unexpected cell {x},{y}: {cp:?}");
            }
            let position = CellPosition {
                x: u16::try_from(x).unwrap(),
                y: pin.y,
            };
            for &scalar in page.lookup_grapheme(position).unwrap_or_default() {
                assert_eq!(chars.next(), Some(scalar), "suffix at {x},{y}");
            }
        }
        assert_eq!(chars.next(), None, "unmatched text on row {y}");
    }
    assert_eq!(lines.next(), None, "expected text exceeds active rows");
}

pub(crate) fn row(t: &Terminal, y: u32) -> maestro_terminal::page::row::Row {
    let screen = t.screens.active();
    let pin = screen
        .pages
        .pin(Point::Active(Coordinate { x: 0, y }))
        .unwrap();
    *pin.row_and_cell(&screen.pages).unwrap().0
}

pub(crate) fn suffix(t: &Terminal, x: u16, y: u32) -> Option<&[char]> {
    let screen = t.screens.active();
    let pin = screen
        .pages
        .pin(Point::Active(Coordinate { x, y }))
        .unwrap();
    screen
        .pages
        .page(pin.node)
        .unwrap()
        .lookup_grapheme(CellPosition { x, y: pin.y })
}
