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

#[test]
fn terminal_grapheme_growth_retains_cursor_references() {
    let mut t = Terminal::new(80, 24, 0).unwrap();
    t.set_attribute(Attribute::Bold).unwrap();
    t.screens
        .active_mut()
        .start_hyperlink(b"https://example.test/cluster", Some(b"cluster"))
        .unwrap();
    t.print('o').unwrap();
    let handle = t.screens.active().cursor.page_pin;
    let initial = *t.screens.active().pages.tracked_pin(handle).unwrap();
    let expected = t.screens.active().cursor.clone();
    let capacity = t
        .screens
        .active()
        .pages
        .page(initial.node)
        .unwrap()
        .capacity
        .grapheme_bytes;
    let position = crate::page::CellPosition { x: 0, y: initial.y };
    let page = t.screens.active_mut().pages.page_mut(initial.node).unwrap();
    let mut count = 0;
    while page.append_grapheme(position, '\u{301}').is_ok() {
        count += 1;
    }
    page.get_row_mut(initial.y).set_dirty(false);
    t.print('\u{301}').unwrap();
    assert!(dirty(&t, 0));
    let screen = t.screens.active();
    let pin = *screen.pages.tracked_pin(handle).unwrap();
    assert_ne!(pin.node, initial.node);
    assert!(screen.pages.page(initial.node).is_none());
    assert_eq!(screen.cursor.page_pin, handle);
    assert_eq!((screen.cursor.x, screen.cursor.y), (1, 0));
    let page = screen.pages.page(pin.node).unwrap();
    assert!(page.capacity.grapheme_bytes > capacity);
    assert_eq!(
        page.lookup_grapheme(crate::page::CellPosition { x: 0, y: pin.y })
            .unwrap(),
        vec!['\u{301}'; count + 1]
    );
    assert_eq!(page.get_cells(pin.y)[0].codepoint(), 'o');
    assert_eq!(
        page.get_row(pin.y).grapheme(),
        cell(&t, 0, 0).has_grapheme()
    );
    assert_cursor_references(screen, &expected);
}

/// Check cell and cursor ownership after the page replacement.
fn assert_cursor_references(screen: &crate::Screen, expected: &crate::screen::Cursor) {
    let pin = screen.pages.tracked_pin(screen.cursor.page_pin).unwrap();
    let page = screen.pages.page(pin.node).unwrap();
    assert_eq!(screen.cursor.style, expected.style);
    assert_eq!(*page.styles.get(screen.cursor.style_id), expected.style);
    assert_eq!(page.styles.ref_count(screen.cursor.style_id), 2);
    assert_eq!(screen.cursor.hyperlink, expected.hyperlink);
    assert_eq!(
        page.hyperlink_set.get(screen.cursor.hyperlink_id),
        expected.hyperlink.as_deref().unwrap()
    );
    assert_eq!(page.hyperlink_set.ref_count(screen.cursor.hyperlink_id), 2);
}

#[test]
fn cursor_state_restorecursor_uses_default_style_on_outofspace() {
    use crate::{
        color::Rgb,
        page_list::IncreaseCapacity,
        style::{Color, Style},
    };
    let mut t = Terminal::new(10, 1, 0).unwrap();
    t.set_attribute(Attribute::Bold).unwrap();
    t.save_cursor();
    t.set_attribute(Attribute::Unset).unwrap();
    assert!(!t.screens.active().cursor.style.flags.bold());
    loop {
        let screen = t.screens.active_mut();
        let pin = *screen.pages.tracked_pin(screen.cursor.page_pin).unwrap();
        if screen.pages.page(pin.node).unwrap().capacity.styles == u16::MAX {
            break;
        }
        screen
            .increase_capacity(pin.node, Some(IncreaseCapacity::Styles))
            .unwrap();
    }
    let screen = t.screens.active_mut();
    let pin = *screen.pages.tracked_pin(screen.cursor.page_pin).unwrap();
    let page = screen.pages.page_mut(pin.node).unwrap();
    assert_eq!(page.capacity.styles, u16::MAX);
    for n in 1..=u16::MAX {
        let [r, g] = n.to_be_bytes();
        if page
            .styles
            .add(Style {
                bg_color: Color::Rgb(Rgb { r, g, b: 0 }),
                ..Style::default()
            })
            .is_err()
        {
            break;
        }
    }
    t.restore_cursor();
    assert!(!t.screens.active().cursor.style.flags.bold());
    assert_eq!(t.screens.active().cursor.style_id, crate::style::DEFAULT_ID);
}

#[test]
fn line_control_index_bottom_of_scroll_region_clear_hyperlinks() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.scrolling_region.top = 1;
    t.scrolling_region.bottom = 2;
    t.set_cursor_pos(2, 1);
    t.screens
        .active_mut()
        .start_hyperlink(b"http://example.com", None)
        .unwrap();
    t.print('A').unwrap();
    t.screens.active_mut().end_hyperlink();
    t.index().unwrap();
    t.carriage_return();
    t.print('B').unwrap();
    t.index().unwrap();
    t.carriage_return();
    t.print('C').unwrap();
    for y in 0..5 {
        for x in 0..5 {
            assert_eq!(
                cell(&t, x, y).codepoint(),
                match (x, y) {
                    (0, 1) => 'B',
                    (0, 2) => 'C',
                    _ => '\0',
                }
            );
        }
    }
    let screen = t.screens.active();
    for y in 1..3 {
        let pin = screen
            .pages
            .pin(Point::Viewport(Coordinate { x: 0, y }))
            .unwrap();
        let (row, cell) = pin.row_and_cell(&screen.pages).unwrap();
        let page = screen.pages.page(pin.node).unwrap();
        assert_eq!((row.hyperlink(), cell.hyperlink()), (false, false));
        assert_eq!(
            page.lookup_hyperlink(crate::page::CellPosition { x: 0, y: pin.y }),
            None
        );
        assert_eq!(page.hyperlink_set.count(), 0);
    }
}

#[test]
fn screen_render_over_wide_char_with_bg_color() {
    let mut t = Terminal::new(80, 80, 0).unwrap();
    t.set_attribute(Attribute::DirectColorBg(crate::color::Rgb {
        r: 255,
        g: 0,
        b: 0,
    }))
    .unwrap();
    t.print('😀').unwrap();
    assert_eq!(cursor_page(&t).styles.count(), 1);
    t.set_cursor_pos(0, 0);
    t.set_attribute(Attribute::Unset).unwrap();
    t.print('A').unwrap();
    assert_eq!(cursor_page(&t).styles.count(), 0);
    assert!(dirty(&t, 0));
}

/// Borrow the page owning the active cursor.
fn cursor_page(t: &Terminal) -> &crate::Page {
    let screen = t.screens.active();
    let pin = screen.pages.tracked_pin(screen.cursor.page_pin).unwrap();
    screen.pages.page(pin.node).unwrap()
}

#[test]
fn screen_input_glitch_text() {
    let mut t = Terminal::new(30, 30, 0).unwrap();
    let capacity = first_grapheme_capacity(&t);
    while first_grapheme_capacity(&t) == capacity {
        t.print_string(include_str!("../../../tests/fixtures/glitch.txt"))
            .unwrap();
    }
    assert!(first_grapheme_capacity(&t) > capacity);
}

/// Reload the oldest page after a storage replacement.
fn first_grapheme_capacity(t: &Terminal) -> u32 {
    let pages = &t.screens.active().pages;
    let pin = pages.pin(Point::Screen(Coordinate { x: 0, y: 0 })).unwrap();
    pages.page(pin.node).unwrap().capacity.grapheme_bytes
}

/// Check a row's linked cell and its retained identity.
fn assert_link(t: &Terminal, x: u16, id: Option<crate::links::Id>) {
    let screen = t.screens.active();
    let pin = screen
        .pages
        .pin(Point::Active(Coordinate { x, y: 0 }))
        .unwrap();
    let (row, cell) = pin.row_and_cell(&screen.pages).unwrap();
    let page = screen.pages.page(pin.node).unwrap();
    assert!(row.hyperlink());
    assert_eq!(cell.hyperlink(), id.is_some());
    assert_eq!(
        page.lookup_hyperlink(crate::page::CellPosition { x, y: pin.y }),
        id
    );
}

#[test]
fn screen_render_with_hyperlink() {
    let mut t = Terminal::new(80, 80, 0).unwrap();
    t.screens
        .active_mut()
        .start_hyperlink(b"http://example.com", None)
        .unwrap();
    t.print_string("123456").unwrap();
    for x in 0..6 {
        assert_link(&t, x, Some(1));
    }
    assert!(dirty(&t, 0));
}

#[test]
fn screen_render_over_cell_with_same_hyperlink() {
    let mut t = Terminal::new(80, 80, 0).unwrap();
    t.screens
        .active_mut()
        .start_hyperlink(b"http://example.com", None)
        .unwrap();
    t.print_string("123456").unwrap();
    t.set_cursor_pos(1, 1);
    t.print_string("123456").unwrap();
    for x in 0..6 {
        assert_link(&t, x, Some(1));
    }
    assert!(dirty(&t, 0));
}

#[test]
fn screen_render_and_end_hyperlink() {
    let mut t = Terminal::new(80, 80, 0).unwrap();
    t.screens
        .active_mut()
        .start_hyperlink(b"http://example.com", None)
        .unwrap();
    t.print_string("123").unwrap();
    t.screens.active_mut().end_hyperlink();
    t.print_string("456").unwrap();
    for x in 0..3 {
        assert_link(&t, x, Some(1));
    }
    for x in 3..6 {
        assert_link(&t, x, None);
    }
    assert!(dirty(&t, 0));
}

#[test]
fn screen_render_and_change_hyperlink() {
    let mut t = Terminal::new(80, 80, 0).unwrap();
    t.screens
        .active_mut()
        .start_hyperlink(b"http://one.example.com", None)
        .unwrap();
    t.print_string("123").unwrap();
    t.screens
        .active_mut()
        .start_hyperlink(b"http://two.example.com", None)
        .unwrap();
    t.print_string("456").unwrap();
    for x in 0..3 {
        assert_link(&t, x, Some(1));
    }
    for x in 3..6 {
        assert_link(&t, x, Some(2));
    }
    assert!(dirty(&t, 0));
}

#[test]
fn screen_overwrite_hyperlink() {
    let mut t = Terminal::new(80, 80, 0).unwrap();
    t.screens
        .active_mut()
        .start_hyperlink(b"http://one.example.com", None)
        .unwrap();
    t.print_string("123").unwrap();
    t.set_cursor_pos(1, 1);
    t.screens.active_mut().end_hyperlink();
    t.print_string("456").unwrap();
    let screen = t.screens.active();
    for x in 0..3 {
        let pin = screen
            .pages
            .pin(Point::Active(Coordinate { x, y: 0 }))
            .unwrap();
        let (row, cell) = pin.row_and_cell(&screen.pages).unwrap();
        let page = screen.pages.page(pin.node).unwrap();
        assert!(!row.hyperlink());
        assert!(!cell.hyperlink());
        assert_eq!(
            page.lookup_hyperlink(crate::page::CellPosition { x, y: pin.y }),
            None
        );
        assert_eq!(page.hyperlink_set.count(), 0);
    }
    assert!(dirty(&t, 0));
}

#[test]
fn row_shifts_retain_traversal_across_destination_growth() {
    for insert in [false, true] {
        let mut t = Terminal::new(600, 100, 0).unwrap();
        let pages = &t.screens.active().pages;
        let origin = pages.get_top_left(crate::point::Tag::Active);
        let boundary = (2..98)
            .find(|y| origin.down(pages, *y).unwrap().y == 0)
            .unwrap();
        let source = if insert { boundary - 1 } else { boundary };
        let destination = if insert { boundary } else { boundary - 1 };
        t.screens
            .active_mut()
            .cursor_absolute(0, u16::try_from(source).unwrap());
        t.set_attribute(Attribute::Bold).unwrap();
        for _ in 0..600 {
            t.print_string("a\u{301}").unwrap();
        }
        let pages = &t.screens.active().pages;
        let old = pages
            .get_top_left(crate::point::Tag::Active)
            .down(pages, destination)
            .unwrap()
            .node;
        let tracked_count = t.screens.active().pages.tracked_pins().count();
        t.scrolling_region.top = u16::try_from(boundary - 2).unwrap();
        t.scrolling_region.bottom = u16::try_from(boundary + 2).unwrap();
        t.screens
            .active_mut()
            .cursor_absolute(0, t.scrolling_region.top);
        if insert {
            t.insert_lines(1).unwrap();
        } else {
            t.delete_lines(1).unwrap();
        }
        let pages = &t.screens.active().pages;
        assert!(pages.page(old).is_none(), "destination must have grown");
        assert_eq!(pages.tracked_pins().count(), tracked_count);
        let pin = pages
            .pin(Point::Active(Coordinate {
                x: 0,
                y: u32::try_from(destination).unwrap(),
            }))
            .unwrap();
        let page = pages.page(pin.node).unwrap();
        for x in 0..600 {
            assert_eq!(page.get_cells(pin.y)[usize::from(x)].codepoint(), 'a');
            assert_eq!(
                page.lookup_grapheme(crate::page::CellPosition { x, y: pin.y }),
                Some(&['\u{301}'][..])
            );
            assert!(
                page.styles
                    .get(page.get_cells(pin.y)[usize::from(x)].style_id())
                    .flags
                    .bold()
            );
        }
        assert!(pin.is_dirty(pages).unwrap());
    }
}
