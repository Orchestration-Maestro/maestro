//! Printing assertions requiring page-local ownership counts.

use crate::{Coordinate, Point, Terminal, page::Cell, sgr::Attribute};

fn cell(t: &Terminal, x: u16, y: u32) -> Cell {
    let s = t.screens.active();
    let pin = s.pages.pin(Point::Active(Coordinate { x, y })).unwrap();
    *pin.row_and_cell(&s.pages).unwrap().1
}

fn dirty(t: &Terminal, y: u32) -> bool {
    let s = t.screens.active();
    s.pages
        .pin(Point::Active(Coordinate { x: 0, y }))
        .unwrap()
        .is_dirty(&s.pages)
        .unwrap()
}

#[test]
fn screen_render_over_wide_char_with_bold() {
    let mut t = Terminal::new(80, 80, 0).unwrap();

    t.set_attribute(Attribute::Bold).unwrap();
    t.print(char::from_u32(0x1F600).unwrap()).unwrap();

    {
        let screen = t.screens.active();
        let pin = screen.pages.tracked_pin(screen.cursor.page_pin).unwrap();
        let page = screen.pages.page(pin.node).unwrap();
        assert_eq!(page.styles.count(), 1);
    }

    t.set_cursor_pos(0, 0);
    t.set_attribute(Attribute::Unset).unwrap();
    t.print('A').unwrap();

    {
        let screen = t.screens.active();
        let pin = screen.pages.tracked_pin(screen.cursor.page_pin).unwrap();
        let page = screen.pages.page(pin.node).unwrap();
        assert_eq!(page.styles.count(), 0);
    }

    assert!(dirty(&t, 0));
}

#[test]
fn screen_bold_style() {
    let mut t = Terminal::new(5, 5, 0).unwrap();

    t.set_attribute(Attribute::Bold).unwrap();
    t.print('A').unwrap();

    {
        let value = cell(&t, 0, 0);

        assert_eq!(value.codepoint(), 'A');
        assert_ne!(value.style_id(), 0);
        let screen = t.screens.active();
        let pin = screen.pages.tracked_pin(screen.cursor.page_pin).unwrap();
        let page = screen.pages.page(pin.node).unwrap();
        assert!(page.styles.ref_count(t.screens.active().cursor.style_id) > 1);
    }
}

#[test]
fn screen_garbage_collect_overwritten() {
    let mut t = Terminal::new(5, 5, 0).unwrap();

    t.set_attribute(Attribute::Bold).unwrap();
    t.print('A').unwrap();
    t.set_cursor_pos(1, 1);
    t.set_attribute(Attribute::Unset).unwrap();
    t.print('B').unwrap();

    {
        let value = cell(&t, 0, 0);

        assert_eq!(value.codepoint(), 'B');
        assert_eq!(value.style_id(), 0);
    }

    let screen = t.screens.active();
    let pin = screen.pages.tracked_pin(screen.cursor.page_pin).unwrap();
    let page = screen.pages.page(pin.node).unwrap();
    assert_eq!(page.styles.count(), 0);
}

#[test]
fn screen_do_not_garbage_collect_old_styles_in_use() {
    let mut t = Terminal::new(5, 5, 0).unwrap();

    t.set_attribute(Attribute::Bold).unwrap();
    t.print('A').unwrap();
    t.set_attribute(Attribute::Unset).unwrap();
    t.print('B').unwrap();

    {
        let value = cell(&t, 1, 0);

        assert_eq!(value.codepoint(), 'B');
        assert_eq!(value.style_id(), 0);
    }

    let screen = t.screens.active();
    let pin = screen.pages.tracked_pin(screen.cursor.page_pin).unwrap();
    let page = screen.pages.page(pin.node).unwrap();
    assert_eq!(page.styles.count(), 1);
}
