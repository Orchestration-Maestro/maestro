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

pub(crate) fn plain(t: &Terminal) -> String {
    let screen = t.screens.active();
    let mut lines = Vec::new();
    for y in 0..u32::from(t.rows()) {
        let pin = screen
            .pages
            .pin(Point::Active(Coordinate { x: 0, y }))
            .unwrap();
        let page = screen.pages.page(pin.node).unwrap();
        let mut line = String::new();
        for (x, cell) in page.get_cells(pin.y).iter().enumerate() {
            if matches!(cell.wide(), Wide::SpacerTail | Wide::SpacerHead) {
                continue;
            }
            let cp = cell.codepoint();
            line.push(if cp == '\0' { ' ' } else { cp });
            if cell.has_grapheme() {
                line.extend(
                    page.lookup_grapheme(CellPosition {
                        x: u16::try_from(x).unwrap(),
                        y: pin.y,
                    })
                    .unwrap(),
                );
            }
        }
        lines.push(line.trim_end_matches(' ').to_owned());
    }
    while lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    lines.join("\n")
}

pub(crate) fn row(t: &Terminal, y: u32) -> maestro_terminal::page::row::Row {
    let screen = t.screens.active();
    let pin = screen
        .pages
        .pin(Point::Active(Coordinate { x: 0, y }))
        .unwrap();
    *pin.row_and_cell(&screen.pages).unwrap().0
}
