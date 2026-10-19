//! Clearing, managed neighbors and wide-cell boundary behavior.

use super::{page, pin, screen, write_row};
use crate::{Cell, Coordinate, Point, page::cell::Wide, sgr::Attribute, style::Color};

#[test]
fn screen_clear_rows_active_one_line() {
    let mut s = screen(20, 3);
    write_row(&mut s, "hello, world");
    s.clear_rows(Point::Active(Coordinate::default()), None, false);
    assert!(page(&s).get_row(0).dirty());
    assert!(page(&s).get_cells(0).iter().all(|cell| cell.is_zero()));
}

#[test]
fn screen_clear_rows_active_multi_line() {
    let mut s = screen(20, 3);
    write_row(&mut s, "hello");
    s.cursor_absolute(0, 1);
    write_row(&mut s, "world");
    s.clear_rows(Point::Active(Coordinate::default()), None, false);
    for y in 0..3 {
        assert!(page(&s).get_row(y).dirty());
        assert!(page(&s).get_cells(y).iter().all(|cell| cell.is_zero()));
    }
}

#[test]
fn screen_clear_rows_active_styled_line() {
    let mut s = screen(20, 3);
    s.set_attribute(Attribute::Bold).unwrap();
    write_row(&mut s, "hello world");
    s.set_attribute(Attribute::Unset).unwrap();
    assert_eq!(page(&s).styles.count(), 1);
    s.clear_rows(Point::Active(Coordinate::default()), None, false);
    assert_eq!(page(&s).styles.count(), 0);
    assert!(page(&s).get_cells(0).iter().all(|cell| cell.is_zero()));
}

#[test]
fn screen_clear_cells_empty_range() {
    let mut s = screen(5, 1);
    write_row(&mut s, "hello");
    let before = page(&s).get_cells(0).to_vec();
    let row = *page(&s).get_row(0);
    s.clear_cells(pin(&s).node, 0, 0..0);
    assert_eq!(page(&s).get_cells(0), before);
    assert_eq!(*page(&s).get_row(0), row);
}

#[test]
fn screen_clear_rows_uses_stored_page_width() {
    let mut s = screen(4, 1);
    write_row(&mut s, "AB");
    let node = pin(&s).node;
    s.pages.page_mut(node).unwrap().size.cols = 2;
    s.cursor_horizontal_absolute(0);
    s.clear_rows(Point::Screen(Coordinate::default()), None, false);
    assert_eq!(page(&s).get_cells(0).len(), 2);
    assert!(page(&s).get_cells(0).iter().all(|cell| cell.is_zero()));
}

#[test]
fn screen_clear_rows_protected() {
    let mut s = screen(8, 2);
    for y in 0..2 {
        s.cursor_absolute(0, y);
        write_row(&mut s, "ABCDEFGH");
        let page = s.pages.page_mut(pin(&s).node).unwrap();
        for x in 0..8 {
            page.get_cells_mut(y)[x].set_protected((x / 2 + usize::from(y)) % 2 == 0);
        }
        page.get_row_mut(y).set_wrap(true);
    }
    s.clear_rows(Point::Active(Coordinate::default()), None, true);
    for y in 0..2 {
        for (x, cell) in page(&s).get_cells(y).iter().enumerate() {
            assert_eq!(cell.is_zero(), (x / 2 + usize::from(y)) % 2 != 0);
        }
        assert!(page(&s).get_row(y).wrap());
        assert!(page(&s).get_row(y).dirty());
    }
}

#[test]
fn screen_blank_cell_keeps_only_background() {
    let mut s = screen(5, 1);
    assert!(s.blank_cell().is_zero());
    s.set_attribute(Attribute::Bold).unwrap();
    s.set_attribute(Attribute::Palette256Fg(2)).unwrap();
    assert!(s.blank_cell().is_zero());
    s.set_attribute(Attribute::Palette256Bg(3)).unwrap();
    let blank = s.blank_cell();
    assert_eq!(blank, s.cursor.style.bg_cell().unwrap());
    assert_eq!(blank.style_id(), 0);
    assert!(!blank.protected());
    assert!(!blank.hyperlink());
}

#[test]
fn screen_protected_clear_preserves_managed_neighbours() {
    let mut s = screen(5, 1);
    s.set_attribute(Attribute::Bold).unwrap();
    s.start_hyperlink(b"https://example.test", None).unwrap();
    write_row(&mut s, "ABCDE");
    let p = pin(&s);
    let style = s.cursor.style_id;
    let link = s.cursor.hyperlink_id;
    {
        let page = s.pages.page_mut(p.node).unwrap();
        page.get_cells_mut(0)[2].set_protected(true);
        for x in 1..4 {
            page.append_grapheme(crate::page::CellPosition { x, y: 0 }, '\u{301}')
                .unwrap();
        }
    }
    s.set_attribute(Attribute::Palette256Bg(3)).unwrap();
    s.clear_unprotected_cells(p.node, 0, 1..4);
    let page = page(&s);
    assert_eq!(page.styles.ref_count(style), 3);
    assert_eq!(page.hyperlink_set.ref_count(link), 4);
    assert_protected_neighbours(&s, style);
    assert_eq!(
        page.lookup_grapheme(crate::page::CellPosition { x: 2, y: 0 }),
        Some(['\u{301}'].as_slice())
    );
    for x in [1, 3] {
        assert!(
            page.lookup_grapheme(crate::page::CellPosition { x, y: 0 })
                .is_none()
        );
    }
    assert!(page.get_row(0).grapheme());
    assert!(page.get_row(0).styled());
    assert!(page.get_row(0).hyperlink());
    assert_eq!(s.cursor.style.bg_color, Color::Palette(3));
}

/// Verify retained neighbor styles and links alongside background-only cleared cells.
fn assert_protected_neighbours(s: &super::Screen, style: crate::style::Id) {
    for x in [0, 2, 4] {
        assert_eq!(page(s).get_cells(0)[x].style_id(), style);
        assert!(page(s).get_cells(0)[x].hyperlink());
    }
    for x in [1, 3] {
        assert_eq!(page(s).get_cells(0)[x], s.blank_cell());
    }
}

#[test]
fn screen_wrap_reset_clears_only_wrap_and_spacer_state() {
    let mut s = screen(4, 2);
    write_row(&mut s, "ABCD");
    let p = pin(&s);
    {
        let page = s.pages.page_mut(p.node).unwrap();
        page.get_row_mut(0).set_wrap(true);
        page.get_row_mut(1).set_wrap_continuation(true);
        page.get_cells_mut(0)[3].set_wide(Wide::SpacerHead);
    }
    s.cursor.pending_wrap = true;
    s.cursor_reset_wrap();
    assert!(!s.cursor.pending_wrap);
    assert!(!page(&s).get_row(0).wrap());
    assert!(!page(&s).get_row(1).wrap_continuation());
    assert_eq!(page(&s).get_cells(0)[0], Cell::init('A'));
    assert!(page(&s).get_cells(0)[3].is_zero());
    s.pages.page_mut(p.node).unwrap().get_cells_mut(0)[3].set_wide(Wide::SpacerHead);
    s.cursor_reset_wrap();
    assert_eq!(page(&s).get_cells(0)[3].wide(), Wide::SpacerHead);
}

#[test]
fn screen_wide_boundary_split_clears_only_disconnected_halves() {
    for boundary in 0..=4 {
        let mut s = screen(4, 3);
        let p = pin(&s);
        {
            let page = s.pages.page_mut(p.node).unwrap();
            page.get_row_mut(0).set_wrap(true);
            page.get_cells_mut(0)[3].set_wide(Wide::SpacerHead);
            page.get_row_mut(1).set_wrap_continuation(true);
            page.get_row_mut(1).set_wrap(true);
            page.get_cells_mut(1)[0] = Cell::init('界');
            page.get_cells_mut(1)[0].set_wide(Wide::Wide);
            page.get_cells_mut(1)[1].set_wide(Wide::SpacerTail);
            page.get_cells_mut(1)[2] = Cell::init('X');
            page.get_cells_mut(1)[3].set_wide(Wide::SpacerHead);
        }
        s.cursor_absolute(0, 1);
        s.split_cell_boundary(boundary);
        assert_eq!(page(&s).get_cells(0)[3].is_zero(), boundary <= 1);
        assert_eq!(page(&s).get_cells(1)[0].is_zero(), boundary == 1);
        assert_eq!(page(&s).get_cells(1)[1].is_zero(), boundary == 1);
        assert_eq!(page(&s).get_cells(1)[2], Cell::init('X'));
        assert_eq!(page(&s).get_cells(1)[3].is_zero(), boundary == 4);
        assert!(page(&s).get_row(1).wrap());
        assert!(page(&s).get_row(1).wrap_continuation());
    }
}
