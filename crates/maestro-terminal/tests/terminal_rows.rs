//! Direct row insertion, deletion and scrolling retain cell ownership.

use super::support::terminal::{assert_text, cell, clear_dirty, dirty, row};
use maestro_terminal::{
    Coordinate, Mode, Point, Terminal, color::Rgb, page::cell::Content, sgr::Attribute,
};

fn lines(t: &mut Terminal, text: &str) {
    for (i, line) in text.split('\n').enumerate() {
        if i != 0 {
            t.carriage_return();
            t.linefeed().unwrap();
        }
        t.print_string(line).unwrap();
    }
}

fn assert_dirty(t: &Terminal, expected: &[bool]) {
    for (y, &value) in expected.iter().enumerate() {
        assert_eq!(dirty(t, u32::try_from(y).unwrap()), value, "row {y}");
    }
}

#[test]
fn editing_insert_lines_simple() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    lines(&mut t, "ABC\nDEF\nGHI");
    t.set_cursor_pos(2, 2);
    clear_dirty(&mut t);
    t.insert_lines(1).unwrap();
    assert_dirty(&t, &[false, true, true, true]);
    assert_text(&t, "ABC\n\nDEF\nGHI");
}

#[test]
fn editing_insert_lines_colors_with_bg_color() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    lines(&mut t, "ABC\nDEF\nGHI");
    t.set_cursor_pos(2, 2);
    let rgb = Rgb { r: 255, g: 0, b: 0 };
    t.screens
        .active_mut()
        .set_attribute(Attribute::DirectColorBg(rgb))
        .unwrap();
    t.insert_lines(1).unwrap();
    assert_text(&t, "ABC\n\nDEF\nGHI");
    for x in 0..t.cols() {
        assert_eq!(cell(&t, x, 1).content(), Content::BgColorRgb(rgb));
    }
}

#[test]
fn editing_insert_lines_handles_style_refs() {
    let mut t = Terminal::new(5, 3, 0).unwrap();
    lines(&mut t, "ABC\nDEF\n");
    t.screens
        .active_mut()
        .set_attribute(Attribute::Bold)
        .unwrap();
    t.print_string("GHI").unwrap();
    t.screens
        .active_mut()
        .set_attribute(Attribute::Unset)
        .unwrap();
    assert_ne!(cell(&t, 0, 2).style_id(), 0);
    t.set_cursor_pos(2, 2);
    t.insert_lines(1).unwrap();
    assert_text(&t, "ABC\n\nDEF");
    for y in 0..3 {
        for x in 0..5 {
            assert_eq!(cell(&t, x, y).style_id(), 0);
        }
    }
}

#[test]
fn editing_insert_lines_outside_of_scroll_region() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    lines(&mut t, "ABC\nDEF\nGHI");
    t.set_top_and_bottom_margin(3, 4);
    t.set_cursor_pos(2, 2);
    clear_dirty(&mut t);
    t.insert_lines(1).unwrap();
    assert_dirty(&t, &[false, false, false]);
    assert_text(&t, "ABC\nDEF\nGHI");
}

#[test]
fn editing_insert_lines_top_bottom_scroll_region() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    lines(&mut t, "ABC\nDEF\nGHI\n123");
    t.set_top_and_bottom_margin(1, 3);
    t.set_cursor_pos(2, 2);
    clear_dirty(&mut t);
    t.insert_lines(1).unwrap();
    assert_dirty(&t, &[false, true, true, false]);
    assert_text(&t, "ABC\n\nDEF\n123");
}

#[test]
fn editing_insert_lines_across_page_boundary_marks_all_shifted_rows_dirty() {
    let mut t = Terminal::new(10, 5, 1024).unwrap();
    let p = t
        .screens
        .active()
        .pages
        .pin(Point::Active(Coordinate { x: 0, y: 0 }))
        .unwrap();
    let capacity = t.screens.active().pages.page(p.node).unwrap().capacity.rows;
    for _ in 0..capacity - 3 {
        t.linefeed().unwrap();
    }
    lines(&mut t, "1AAAA\n2BBBB\n3CCCC\n4DDDD\n5EEEE");
    let pages = &t.screens.active().pages;
    let first = pages.pin(Point::Active(Coordinate { x: 0, y: 0 })).unwrap();
    let last = pages.pin(Point::Active(Coordinate { x: 0, y: 4 })).unwrap();
    assert_ne!(first.node, last.node);
    t.set_cursor_pos(1, 1);
    clear_dirty(&mut t);
    t.insert_lines(1).unwrap();
    assert_dirty(&t, &[true; 5]);
    assert_text(&t, "\n1AAAA\n2BBBB\n3CCCC\n4DDDD");
}

#[test]
fn editing_insert_lines_legacy_test() {
    let mut t = Terminal::new(2, 5, 0).unwrap();
    lines(&mut t, "A\nB\nC\nD\nE");
    t.set_cursor_pos(2, 1);
    t.insert_lines(2).unwrap();
    assert_text(&t, "A\n\n\nB\nC");
}

#[test]
fn editing_insert_lines_zero() {
    let mut t = Terminal::new(2, 5, 0).unwrap();
    t.set_cursor_pos(1, 1);
    t.insert_lines(0).unwrap();
    assert_text(&t, "");
    assert_dirty(&t, &[false; 5]);
}

#[test]
fn editing_insert_lines_with_scroll_region() {
    let mut t = Terminal::new(2, 6, 0).unwrap();
    lines(&mut t, "A\nB\nC\nD\nE");
    t.set_top_and_bottom_margin(1, 2);
    t.set_cursor_pos(1, 1);
    clear_dirty(&mut t);
    t.insert_lines(1).unwrap();
    assert_dirty(&t, &[true, true, false]);
    t.print('X').unwrap();
    assert_text(&t, "X\nA\nC\nD\nE");
}

#[test]
fn editing_insert_lines_more_than_remaining() {
    let mut t = Terminal::new(2, 5, 0).unwrap();
    lines(&mut t, "A\nB\nC\nD\nE");
    t.set_cursor_pos(2, 1);
    clear_dirty(&mut t);
    t.insert_lines(20).unwrap();
    assert_dirty(&t, &[false, true, true]);
    assert_text(&t, "A");
}

#[test]
fn editing_insert_lines_resets_pending_wrap() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.print_string("ABCDE").unwrap();
    assert!(t.screens.active().cursor.pending_wrap);
    t.insert_lines(1).unwrap();
    assert!(!t.screens.active().cursor.pending_wrap);
    t.print('B').unwrap();
    assert_text(&t, "B\nABCDE");
}

#[test]
fn editing_insert_lines_resets_wrap() {
    let mut t = Terminal::new(3, 3, 0).unwrap();
    lines(&mut t, "1\nABCDEF");
    t.set_cursor_pos(1, 1);
    t.insert_lines(1).unwrap();
    t.print('X').unwrap();
    assert_text(&t, "X\n1\nABC");
    assert!(!row(&t, 2).wrap());
}

#[test]
fn editing_insert_lines_multi_codepoint_graphemes() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.modes.set(Mode::GraphemeCluster, true);
    lines(&mut t, "ABC\n👨‍👩‍👧\nGHI");
    t.set_cursor_pos(2, 2);
    t.insert_lines(1).unwrap();
    assert_text(&t, "ABC\n\n👨‍👩‍👧\nGHI");
}

#[test]
fn editing_insert_lines_left_right_scroll_region() {
    let mut t = Terminal::new(10, 10, 0).unwrap();
    lines(&mut t, "ABC123\nDEF456\nGHI789");
    t.scrolling_region.left = 1;
    t.scrolling_region.right = 3;
    t.set_cursor_pos(2, 2);
    clear_dirty(&mut t);
    t.insert_lines(1).unwrap();
    assert_dirty(&t, &[false, true, true, true]);
    assert_text(&t, "ABC123\nD   56\nGEF489\n HI7");
}

fn cursor(t: &Terminal) -> (u16, u16) {
    let c = &t.screens.active().cursor;
    (c.x, c.y)
}

fn assert_links(t: &Terminal, y: u32, expected: &[bool]) {
    let pages = &t.screens.active().pages;
    let pin = pages.pin(Point::Viewport(Coordinate { x: 0, y })).unwrap();
    let page = pages.page(pin.node).unwrap();
    for (x, &linked) in expected.iter().enumerate() {
        let c = page.get_cells(pin.y)[x];
        assert_eq!(c.hyperlink(), linked);
        let id = page.lookup_hyperlink(maestro_terminal::page::CellPosition {
            x: u16::try_from(x).unwrap(),
            y: pin.y,
        });
        assert_eq!(id, linked.then_some(1));
        if linked {
            assert!(page.get_row(pin.y).hyperlink());
        }
    }
}

fn assert_viewport(t: &Terminal, expected: &str) {
    let pages = &t.screens.active().pages;
    let mut actual = Vec::new();
    for y in 0..u32::from(t.rows()) {
        let pin = pages.pin(Point::Viewport(Coordinate { x: 0, y })).unwrap();
        let line: String = pages
            .page(pin.node)
            .unwrap()
            .get_cells(pin.y)
            .iter()
            .map(|c| {
                if c.codepoint() == '\0' {
                    ' '
                } else {
                    c.codepoint()
                }
            })
            .collect();
        actual.push(line.trim_end().to_owned());
    }
    while actual.last().is_some_and(String::is_empty) {
        actual.pop();
    }
    assert_eq!(actual.join("\n"), expected);
}

#[test]
fn editing_scroll_up_simple() {
    let mut t = Terminal::new(5, 5, 10).unwrap();
    lines(&mut t, "ABC\nDEF\nGHI");
    t.set_cursor_pos(2, 2);
    let before = cursor(&t);
    let viewport = t
        .screens
        .active()
        .pages
        .get_top_left(maestro_terminal::point::Tag::Viewport);
    t.scroll_up(1).unwrap();
    assert_eq!(cursor(&t), before);
    assert_ne!(
        t.screens
            .active()
            .pages
            .get_top_left(maestro_terminal::point::Tag::Viewport),
        viewport
    );
    assert_text(&t, "DEF\nGHI");
}

#[test]
fn editing_scroll_up_moves_hyperlink() {
    let mut t = Terminal::new(5, 5, 10).unwrap();
    lines(&mut t, "ABC\n");
    t.screens
        .active_mut()
        .start_hyperlink(b"http://example.com", None)
        .unwrap();
    t.print_string("DEF").unwrap();
    t.screens.active_mut().end_hyperlink();
    lines(&mut t, "\nGHI");
    t.set_cursor_pos(2, 2);
    t.scroll_up(1).unwrap();
    assert_text(&t, "DEF\nGHI");
    assert_links(&t, 0, &[true; 3]);
    assert_links(&t, 1, &[false; 3]);
    assert!(!row(&t, 1).hyperlink());
}

#[test]
fn editing_scroll_up_clears_hyperlink() {
    let mut t = Terminal::new(5, 5, 10).unwrap();
    t.screens
        .active_mut()
        .start_hyperlink(b"http://example.com", None)
        .unwrap();
    t.print_string("ABC").unwrap();
    t.screens.active_mut().end_hyperlink();
    lines(&mut t, "\nDEF\nGHI");
    t.set_cursor_pos(2, 2);
    t.scroll_up(1).unwrap();
    assert_text(&t, "DEF\nGHI");
    assert_links(&t, 0, &[false; 3]);
    assert!(!row(&t, 0).hyperlink());
}

#[test]
fn editing_scroll_up_top_bottom_scroll_region() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    lines(&mut t, "ABC\nDEF\nGHI");
    t.set_top_and_bottom_margin(2, 3);
    t.set_cursor_pos(1, 1);
    clear_dirty(&mut t);
    t.scroll_up(1).unwrap();
    assert_dirty(&t, &[true; 3]);
    assert_text(&t, "ABC\nGHI");
}

#[test]
fn editing_scroll_up_left_right_scroll_region() {
    let mut t = Terminal::new(10, 10, 0).unwrap();
    lines(&mut t, "ABC123\nDEF456\nGHI789");
    t.scrolling_region.left = 1;
    t.scrolling_region.right = 3;
    t.set_cursor_pos(2, 2);
    let before = cursor(&t);
    clear_dirty(&mut t);
    t.scroll_up(1).unwrap();
    assert_eq!(cursor(&t), before);
    assert_dirty(&t, &[true; 3]);
    assert_text(&t, "AEF423\nDHI756\nG   89");
}

#[test]
fn editing_scroll_up_left_right_scroll_region_hyperlink() {
    let mut t = Terminal::new(10, 10, 0).unwrap();
    lines(&mut t, "ABC123\n");
    t.screens
        .active_mut()
        .start_hyperlink(b"http://example.com", None)
        .unwrap();
    t.print_string("DEF456").unwrap();
    t.screens.active_mut().end_hyperlink();
    lines(&mut t, "\nGHI789");
    t.scrolling_region.left = 1;
    t.scrolling_region.right = 3;
    t.set_cursor_pos(2, 2);
    t.scroll_up(1).unwrap();
    assert_text(&t, "AEF423\nDHI756\nG   89");
    assert_links(&t, 0, &[false, true, true, true, false, false]);
    assert_links(&t, 1, &[true, false, false, false, true, true]);
}

#[test]
fn editing_scroll_up_preserves_pending_wrap() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    for (y, c) in [(1, 'A'), (2, 'B'), (3, 'C')] {
        t.set_cursor_pos(y, 5);
        t.print(c).unwrap();
    }
    t.scroll_up(1).unwrap();
    t.print('X').unwrap();
    assert_text(&t, "    B\n    C\n\nX");
}

#[test]
fn editing_scroll_up_full_top_bottom_region() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.print_string("top").unwrap();
    t.set_cursor_pos(5, 1);
    t.print_string("ABCDE").unwrap();
    t.set_top_and_bottom_margin(2, 5);
    clear_dirty(&mut t);
    t.scroll_up(4).unwrap();
    assert_dirty(&t, &[true; 2]);
    assert_text(&t, "top");
}

#[test]
fn editing_scroll_up_full_top_bottomleft_right_scroll_region() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.print_string("top").unwrap();
    t.set_cursor_pos(5, 1);
    t.print_string("ABCDE").unwrap();
    t.modes.set(Mode::EnableLeftAndRightMargin, true);
    t.set_top_and_bottom_margin(2, 5);
    t.set_left_and_right_margin(2, 4);
    clear_dirty(&mut t);
    t.scroll_up(4).unwrap();
    assert_dirty(&t, &[true; 5]);
    assert_text(&t, "top\n\n\n\nA   E");
}

#[test]
fn editing_scroll_up_creates_scrollback_in_primary_screen() {
    let mut t = Terminal::new(5, 5, 10).unwrap();
    lines(&mut t, "AAAAA\nBBBBB\nCCCCC\nDDDDD\nEEEEE");
    clear_dirty(&mut t);
    t.scroll_up(1).unwrap();
    assert!(dirty(&t, u32::from(t.screens.active().cursor.y)));
    assert_text(&t, "BBBBB\nCCCCC\nDDDDD\nEEEEE");
    t.scroll_viewport(maestro_terminal::terminal::ScrollViewport::Top);
    assert_viewport(&t, "AAAAA\nBBBBB\nCCCCC\nDDDDD\nEEEEE");
}

#[test]
fn editing_scroll_up_with_max_scrollback_zero() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    lines(&mut t, "AAAAA\nBBBBB\nCCCCC");
    t.scroll_up(1).unwrap();
    assert_text(&t, "BBBBB\nCCCCC");
    t.scroll_viewport(maestro_terminal::terminal::ScrollViewport::Top);
    assert_viewport(&t, "BBBBB\nCCCCC");
}

#[test]
fn editing_scroll_up_with_max_scrollback_zero_and_top_margin() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    lines(&mut t, "AAAAA\nBBBBB\nCCCCC\nDDDDD");
    t.set_top_and_bottom_margin(2, 5);
    t.scroll_up(1).unwrap();
    assert_text(&t, "AAAAA\nCCCCC\nDDDDD");
}

#[test]
fn editing_scroll_up_with_max_scrollback_zero_and_left_right_margin() {
    let mut t = Terminal::new(10, 5, 0).unwrap();
    lines(&mut t, "AAAAABBBBB\nCCCCCDDDDD\nEEEEEFFFFF");
    t.modes.set(Mode::EnableLeftAndRightMargin, true);
    t.set_left_and_right_margin(2, 6);
    t.scroll_up(1).unwrap();
    assert_text(&t, "ACCCCDBBBB\nCEEEEFDDDD\nE     FFFF");
}

#[test]
fn editing_scroll_down_simple() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    lines(&mut t, "ABC\nDEF\nGHI");
    t.set_cursor_pos(2, 2);
    let before = cursor(&t);
    clear_dirty(&mut t);
    t.scroll_down(1).unwrap();
    assert_eq!(cursor(&t), before);
    assert_dirty(&t, &[true; 5]);
    assert_text(&t, "\nABC\nDEF\nGHI");
}

#[test]
fn editing_scroll_down_hyperlink_moves() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.screens
        .active_mut()
        .start_hyperlink(b"http://example.com", None)
        .unwrap();
    t.print_string("ABC").unwrap();
    t.screens.active_mut().end_hyperlink();
    lines(&mut t, "\nDEF\nGHI");
    t.set_cursor_pos(2, 2);
    t.scroll_down(1).unwrap();
    assert_text(&t, "\nABC\nDEF\nGHI");
    assert_links(&t, 1, &[true; 3]);
    assert_links(&t, 0, &[false; 3]);
    assert!(!row(&t, 0).hyperlink());
}

#[test]
fn editing_scroll_down_outside_of_scroll_region() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    lines(&mut t, "ABC\nDEF\nGHI");
    t.set_top_and_bottom_margin(3, 4);
    t.set_cursor_pos(2, 2);
    let before = cursor(&t);
    clear_dirty(&mut t);
    t.scroll_down(1).unwrap();
    assert_eq!(cursor(&t), before);
    assert_dirty(&t, &[false, true, true, true]);
    assert_text(&t, "ABC\nDEF\n\nGHI");
}

#[test]
fn editing_scroll_down_left_right_scroll_region() {
    let mut t = Terminal::new(10, 10, 0).unwrap();
    lines(&mut t, "ABC123\nDEF456\nGHI789");
    t.scrolling_region.left = 1;
    t.scrolling_region.right = 3;
    t.set_cursor_pos(2, 2);
    let before = cursor(&t);
    clear_dirty(&mut t);
    t.scroll_down(1).unwrap();
    assert_eq!(cursor(&t), before);
    assert_dirty(&t, &[true; 4]);
    assert_text(&t, "A   23\nDBC156\nGEF489\n HI7");
}

#[test]
fn editing_scroll_down_left_right_scroll_region_hyperlink() {
    let mut t = Terminal::new(10, 10, 0).unwrap();
    t.screens
        .active_mut()
        .start_hyperlink(b"http://example.com", None)
        .unwrap();
    t.print_string("ABC123").unwrap();
    t.screens.active_mut().end_hyperlink();
    lines(&mut t, "\nDEF456\nGHI789");
    t.scrolling_region.left = 1;
    t.scrolling_region.right = 3;
    t.set_cursor_pos(2, 2);
    t.scroll_down(1).unwrap();
    assert_text(&t, "A   23\nDBC156\nGEF489\n HI7");
    assert_links(&t, 0, &[true, false, false, false, true, true]);
    assert_links(&t, 1, &[false, true, true, true, false, false]);
}

#[test]
fn editing_scroll_down_outside_of_left_right_scroll_region() {
    let mut t = Terminal::new(10, 10, 0).unwrap();
    lines(&mut t, "ABC123\nDEF456\nGHI789");
    t.scrolling_region.left = 1;
    t.scrolling_region.right = 3;
    t.set_cursor_pos(1, 1);
    let before = cursor(&t);
    clear_dirty(&mut t);
    t.scroll_down(1).unwrap();
    assert_eq!(cursor(&t), before);
    assert_dirty(&t, &[true; 4]);
    assert_text(&t, "A   23\nDBC156\nGEF489\n HI7");
}

#[test]
fn editing_scroll_down_preserves_pending_wrap() {
    let mut t = Terminal::new(5, 10, 0).unwrap();
    for (y, c) in [(1, 'A'), (2, 'B'), (3, 'C')] {
        t.set_cursor_pos(y, 5);
        t.print(c).unwrap();
    }
    t.scroll_down(1).unwrap();
    t.print('X').unwrap();
    assert_text(&t, "\n    A\n    B\nX   C");
}

#[test]
fn editing_reverse_index_top_of_screen() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    lines(&mut t, "A\nB\nC");
    t.set_cursor_pos(1, 1);
    t.reverse_index().unwrap();
    t.print('X').unwrap();
    assert_text(&t, "X\nA\nB\nC");
}

#[test]
fn editing_reverse_index_not_top_of_screen() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    lines(&mut t, "A\nB\nC");
    t.set_cursor_pos(2, 1);
    t.reverse_index().unwrap();
    t.print('X').unwrap();
    assert_text(&t, "X\nB\nC");
}

#[test]
fn editing_index_no_scroll_region_top_of_screen() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.print('A').unwrap();
    clear_dirty(&mut t);
    t.index().unwrap();
    t.print('X').unwrap();
    assert_dirty(&t, &[true; 2]);
    assert_text(&t, "A\n X");
}

#[test]
fn editing_index_bottom_of_primary_screen() {
    let mut t = Terminal::new(5, 5, 10).unwrap();
    t.set_cursor_pos(5, 1);
    t.print('A').unwrap();
    clear_dirty(&mut t);
    t.index().unwrap();
    t.print('X').unwrap();
    assert!(dirty(&t, 3));
    assert!(dirty(&t, 4));
    assert_text(&t, "\n\n\nA\n X");
}

#[test]
fn editing_index_bottom_of_primary_screen_background_sgr() {
    let mut t = Terminal::new(5, 5, 10).unwrap();
    t.set_cursor_pos(5, 1);
    t.print('A').unwrap();
    let rgb = Rgb { r: 255, g: 0, b: 0 };
    t.screens
        .active_mut()
        .set_attribute(Attribute::DirectColorBg(rgb))
        .unwrap();
    t.index().unwrap();
    assert_text(&t, "\n\n\nA");
    for x in 0..5 {
        assert_eq!(cell(&t, x, 4).content(), Content::BgColorRgb(rgb));
    }
}

#[test]
fn editing_index_bottom_of_primary_screen_with_scroll_region() {
    let mut t = Terminal::new(5, 5, 10).unwrap();
    t.set_top_and_bottom_margin(1, 3);
    t.set_cursor_pos(3, 1);
    t.print('A').unwrap();
    t.set_cursor_pos(5, 1);
    clear_dirty(&mut t);
    for _ in 0..3 {
        t.index().unwrap();
    }
    t.print('X').unwrap();
    assert_dirty(&t, &[false, false, false, false, true]);
    assert_text(&t, "\n\nA\n\nX");
}

#[test]
fn editing_index_bottom_of_alt_screen_full_region() {
    let mut t = Terminal::new(5, 3, 10).unwrap();
    t.switch_screen_mode(maestro_terminal::terminal::SwitchScreenMode::Mode1049, true)
        .unwrap();
    t.print_string("A\nB\nC").unwrap();
    t.index().unwrap();
    t.carriage_return();
    t.print('D').unwrap();
    assert_text(&t, "B\nC\nD");
    assert_eq!(t.total_rows(), 3);
    t.scroll_viewport(maestro_terminal::terminal::ScrollViewport::Top);
    assert_viewport(&t, "B\nC\nD");
    t.switch_screen_mode(
        maestro_terminal::terminal::SwitchScreenMode::Mode1049,
        false,
    )
    .unwrap();
    assert_text(&t, "");
}

#[test]
fn editing_index_bottom_of_alt_screen_top_region() {
    let mut t = Terminal::new(5, 5, 10).unwrap();
    t.switch_screen_mode(maestro_terminal::terminal::SwitchScreenMode::Mode1049, true)
        .unwrap();
    t.print_string("1\n2\n3\n4\n5").unwrap();
    t.set_top_and_bottom_margin(1, 4);
    t.set_cursor_pos(4, 1);
    t.index().unwrap();
    t.print('X').unwrap();
    assert_text(&t, "2\n3\n4\nX\n5");
    assert_eq!(t.total_rows(), 5);
    t.scroll_viewport(maestro_terminal::terminal::ScrollViewport::Top);
    assert_viewport(&t, "2\n3\n4\nX\n5");
}

#[test]
fn editing_scroll_up_top_region_no_scrollback() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.print_string("A\nB\nC\nD\nE").unwrap();
    t.set_top_and_bottom_margin(1, 3);
    t.scroll_up(1).unwrap();
    assert_text(&t, "B\nC\n\nD\nE");
    assert_eq!(t.total_rows(), 5);
    t.scroll_viewport(maestro_terminal::terminal::ScrollViewport::Top);
    assert_viewport(&t, "B\nC\n\nD\nE");
}

#[test]
fn editing_delete_lines_simple() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    lines(&mut t, "ABC\nDEF\nGHI");
    t.set_cursor_pos(2, 2);
    clear_dirty(&mut t);
    t.delete_lines(1).unwrap();
    assert_dirty(&t, &[false, true, true, true]);
    assert_text(&t, "ABC\nGHI");
}

#[test]
fn editing_delete_lines_colors_with_bg_color() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    lines(&mut t, "ABC\nDEF\nGHI");
    t.set_cursor_pos(2, 2);
    let rgb = Rgb { r: 255, g: 0, b: 0 };
    t.screens
        .active_mut()
        .set_attribute(Attribute::DirectColorBg(rgb))
        .unwrap();
    t.delete_lines(1).unwrap();
    assert_text(&t, "ABC\nGHI");
    for x in 0..5 {
        assert_eq!(cell(&t, x, 4).content(), Content::BgColorRgb(rgb));
    }
}

#[test]
fn editing_delete_lines_across_page_boundary_marks_all_shifted_rows_dirty() {
    let mut t = Terminal::new(10, 5, 1024).unwrap();
    let p = t
        .screens
        .active()
        .pages
        .pin(Point::Active(Coordinate { x: 0, y: 0 }))
        .unwrap();
    let capacity = t.screens.active().pages.page(p.node).unwrap().capacity.rows;
    for _ in 0..capacity - 3 {
        t.linefeed().unwrap();
    }
    lines(&mut t, "1AAAA\n2BBBB\n3CCCC\n4DDDD\n5EEEE");
    let pages = &t.screens.active().pages;
    let first = pages.pin(Point::Active(Coordinate { x: 0, y: 0 })).unwrap();
    let last = pages.pin(Point::Active(Coordinate { x: 0, y: 4 })).unwrap();
    assert_ne!(first.node, last.node);
    t.set_cursor_pos(1, 1);
    clear_dirty(&mut t);
    t.delete_lines(1).unwrap();
    assert_dirty(&t, &[true; 5]);
    assert_text(&t, "2BBBB\n3CCCC\n4DDDD\n5EEEE");
}

#[test]
fn editing_delete_lines_legacy() {
    let mut t = Terminal::new(80, 80, 0).unwrap();
    lines(&mut t, "A\nB\nC\nD");
    t.cursor_up(2);
    t.delete_lines(1).unwrap();
    t.print('E').unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    assert_eq!(cursor(&t), (0, 2));
    assert_text(&t, "A\nE\nD");
}

#[test]
fn editing_delete_lines_with_scroll_region() {
    let mut t = Terminal::new(80, 80, 0).unwrap();
    lines(&mut t, "A\nB\nC\nD");
    t.set_top_and_bottom_margin(1, 3);
    t.set_cursor_pos(1, 1);
    clear_dirty(&mut t);
    t.delete_lines(1).unwrap();
    assert_dirty(&t, &[true, true, true, false]);
    t.print('E').unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    assert_text(&t, "E\nC\n\nD");
}

#[test]
fn editing_delete_lines_with_scroll_region_large_count() {
    let mut t = Terminal::new(80, 80, 0).unwrap();
    lines(&mut t, "A\nB\nC\nD");
    t.set_top_and_bottom_margin(1, 3);
    t.set_cursor_pos(1, 1);
    clear_dirty(&mut t);
    t.delete_lines(5).unwrap();
    assert_dirty(&t, &[true, true, true, false]);
    t.print('E').unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    assert_text(&t, "E\n\n\nD");
}

#[test]
fn editing_delete_lines_with_scroll_region_cursor_outside_of_region() {
    let mut t = Terminal::new(80, 80, 0).unwrap();
    lines(&mut t, "A\nB\nC\nD");
    t.set_top_and_bottom_margin(1, 3);
    t.set_cursor_pos(4, 1);
    clear_dirty(&mut t);
    t.delete_lines(1).unwrap();
    assert_dirty(&t, &[false; 4]);
    assert_text(&t, "A\nB\nC\nD");
}

#[test]
fn editing_delete_lines_resets_pending_wrap() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.print_string("ABCDE").unwrap();
    assert!(t.screens.active().cursor.pending_wrap);
    t.delete_lines(1).unwrap();
    assert!(!t.screens.active().cursor.pending_wrap);
    t.print('B').unwrap();
    assert_text(&t, "B");
}

#[test]
fn editing_delete_lines_resets_wrap() {
    let mut t = Terminal::new(3, 3, 0).unwrap();
    lines(&mut t, "1\nABCDEF");
    t.set_top_and_bottom_margin(1, 2);
    t.set_cursor_pos(1, 1);
    t.delete_lines(1).unwrap();
    t.print('X').unwrap();
    assert_text(&t, "XBC\n\nDEF");
    for y in 0..3 {
        assert!(!row(&t, y).wrap());
    }
}

#[test]
fn editing_delete_lines_left_right_scroll_region() {
    let mut t = Terminal::new(10, 10, 0).unwrap();
    lines(&mut t, "ABC123\nDEF456\nGHI789");
    t.scrolling_region.left = 1;
    t.scrolling_region.right = 3;
    t.set_cursor_pos(2, 2);
    clear_dirty(&mut t);
    t.delete_lines(1).unwrap();
    assert_dirty(&t, &[false, true, true]);
    assert_text(&t, "ABC123\nDHI756\nG   89");
}

#[test]
fn editing_delete_lines_left_right_scroll_region_from_top() {
    let mut t = Terminal::new(10, 10, 0).unwrap();
    lines(&mut t, "ABC123\nDEF456\nGHI789");
    t.scrolling_region.left = 1;
    t.scrolling_region.right = 3;
    t.set_cursor_pos(1, 2);
    clear_dirty(&mut t);
    t.delete_lines(1).unwrap();
    assert_dirty(&t, &[true; 3]);
    assert_text(&t, "AEF423\nDHI756\nG   89");
}

#[test]
fn editing_delete_lines_left_right_scroll_region_high_count() {
    let mut t = Terminal::new(10, 10, 0).unwrap();
    lines(&mut t, "ABC123\nDEF456\nGHI789");
    t.scrolling_region.left = 1;
    t.scrolling_region.right = 3;
    t.set_cursor_pos(2, 2);
    clear_dirty(&mut t);
    t.delete_lines(100).unwrap();
    assert_dirty(&t, &[false, true, true]);
    assert_text(&t, "ABC123\nD   56\nG   89");
}

fn assert_wraps(t: &Terminal, expected: &[bool]) {
    for (y, &value) in expected.iter().enumerate() {
        assert_eq!(row(t, u32::try_from(y).unwrap()).wrap(), value);
    }
}

#[test]
fn editing_delete_lines_wide_character_spacer_head() {
    let mut t = Terminal::new(5, 3, 0).unwrap();
    t.print_string("AAAAABBBB😀CCC").unwrap();
    t.set_cursor_pos(1, 1);
    t.delete_lines(1).unwrap();
    assert_text(&t, "BBBB\n😀CCC");
    assert_wraps(&t, &[false; 3]);
    assert_eq!(
        cell(&t, 4, 0).wide(),
        maestro_terminal::page::cell::Wide::Narrow
    );
}

#[test]
fn editing_delete_lines_wide_character_spacer_head_left_scroll_margin() {
    let mut t = Terminal::new(5, 3, 0).unwrap();
    t.print_string("AAAAABBBB😀CCC").unwrap();
    t.scrolling_region.left = 2;
    t.set_cursor_pos(1, 3);
    t.delete_lines(1).unwrap();
    assert_text(&t, "AABB\nBBCCC\n😀");
    assert_wraps(&t, &[true, true, false]);
    assert_eq!(
        cell(&t, 4, 0).wide(),
        maestro_terminal::page::cell::Wide::Narrow
    );
}

#[test]
fn editing_delete_lines_wide_character_spacer_head_right_scroll_margin() {
    let mut t = Terminal::new(5, 3, 0).unwrap();
    t.print_string("AAAAABBBB😀CCC").unwrap();
    t.scrolling_region.right = 3;
    t.set_cursor_pos(1, 1);
    t.delete_lines(1).unwrap();
    assert_text(&t, "BBBBA\n😀CC\n    C");
    assert_wraps(&t, &[true, true, false]);
    assert_eq!(
        cell(&t, 4, 1).wide(),
        maestro_terminal::page::cell::Wide::Narrow
    );
}

#[test]
fn editing_delete_lines_wide_character_spacer_head_left_and_right_scroll_margin() {
    let mut t = Terminal::new(5, 3, 0).unwrap();
    t.print_string("AAAAABBBB😀CCC").unwrap();
    t.scrolling_region.right = 3;
    t.scrolling_region.left = 2;
    t.set_cursor_pos(1, 3);
    t.delete_lines(1).unwrap();
    assert_text(&t, "AABBA\nBBCC\n😀  C");
    assert_wraps(&t, &[true, true, false]);
    assert_eq!(
        cell(&t, 4, 1).wide(),
        maestro_terminal::page::cell::Wide::SpacerHead
    );
}

#[test]
fn editing_delete_lines_wide_character_spacer_head_left_2_and_right_scroll_margin() {
    let mut t = Terminal::new(5, 3, 0).unwrap();
    t.print_string("AAAAABBBB😀CCC").unwrap();
    t.scrolling_region.right = 3;
    t.scrolling_region.left = 1;
    t.set_cursor_pos(1, 2);
    t.delete_lines(1).unwrap();
    assert_text(&t, "ABBBA\nB CC\n    C");
    assert_wraps(&t, &[true, true, false]);
    assert_eq!(
        cell(&t, 4, 1).wide(),
        maestro_terminal::page::cell::Wide::Narrow
    );
}

#[test]
fn editing_delete_lines_wide_characters_split_by_left_right_scroll_region_boundaries() {
    let mut t = Terminal::new(5, 2, 0).unwrap();
    t.print_string("AAAAA\n😀B😀").unwrap();
    t.scrolling_region.right = 3;
    t.scrolling_region.left = 1;
    t.set_cursor_pos(1, 2);
    t.delete_lines(1).unwrap();
    assert_text(&t, "A B A");
    for y in 0..2 {
        for x in 0..5 {
            assert_eq!(
                cell(&t, x, y).wide(),
                maestro_terminal::page::cell::Wide::Narrow
            );
        }
    }
}

#[test]
fn editing_delete_lines_zero() {
    let mut t = Terminal::new(2, 5, 0).unwrap();
    t.set_cursor_pos(1, 1);
    t.delete_lines(0).unwrap();
    assert_text(&t, "");
    assert_dirty(&t, &[false; 5]);
}

#[test]
fn editing_insert_blanks_zero() {
    let mut t = Terminal::new(5, 2, 0).unwrap();
    t.print_string("ABC").unwrap();
    t.set_cursor_pos(1, 1);
    t.insert_blanks(0);
    assert_text(&t, "ABC");
}

#[test]
fn editing_insert_blanks() {
    let mut t = Terminal::new(5, 2, 0).unwrap();
    t.print_string("ABC").unwrap();
    t.set_cursor_pos(1, 1);
    clear_dirty(&mut t);
    t.insert_blanks(2);
    assert_dirty(&t, &[true, false]);
    assert_text(&t, "  ABC");
}

#[test]
fn editing_insert_blanks_pushes_off_end() {
    let mut t = Terminal::new(3, 2, 0).unwrap();
    t.print_string("ABC").unwrap();
    t.set_cursor_pos(1, 1);
    clear_dirty(&mut t);
    t.insert_blanks(2);
    assert!(dirty(&t, 0));
    assert_text(&t, "  A");
}

#[test]
fn editing_insert_blanks_more_than_size() {
    let mut t = Terminal::new(3, 2, 0).unwrap();
    t.print_string("ABC").unwrap();
    t.set_cursor_pos(1, 1);
    clear_dirty(&mut t);
    t.insert_blanks(5);
    assert!(dirty(&t, 0));
    assert_text(&t, "");
}

#[test]
fn editing_insert_blanks_no_scroll_region_fits() {
    let mut t = Terminal::new(10, 10, 0).unwrap();
    t.print_string("ABC").unwrap();
    t.set_cursor_pos(1, 1);
    clear_dirty(&mut t);
    t.insert_blanks(2);
    assert!(dirty(&t, 0));
    assert_text(&t, "  ABC");
}

#[test]
fn editing_insert_blanks_preserves_background_sgr() {
    let mut t = Terminal::new(10, 10, 0).unwrap();
    t.print_string("ABC").unwrap();
    t.set_cursor_pos(1, 1);
    let rgb = Rgb { r: 255, g: 0, b: 0 };
    t.screens
        .active_mut()
        .set_attribute(Attribute::DirectColorBg(rgb))
        .unwrap();
    t.insert_blanks(2);
    assert_text(&t, "  ABC");
    assert_eq!(cell(&t, 0, 0).content(), Content::BgColorRgb(rgb));
}

#[test]
fn editing_insert_blanks_shift_off_screen() {
    let mut t = Terminal::new(5, 10, 0).unwrap();
    t.print_string("  ABC").unwrap();
    t.set_cursor_pos(1, 3);
    clear_dirty(&mut t);
    t.insert_blanks(2);
    assert!(dirty(&t, 0));
    t.print('X').unwrap();
    assert_text(&t, "  X A");
}

#[test]
fn editing_insert_blanks_split_multi_cell_character() {
    let mut t = Terminal::new(5, 10, 0).unwrap();
    t.print_string("123橋").unwrap();
    t.set_cursor_pos(1, 1);
    clear_dirty(&mut t);
    t.insert_blanks(1);
    assert!(dirty(&t, 0));
    assert_text(&t, " 123");
}

#[test]
fn editing_insert_blanks_inside_left_right_scroll_region() {
    let mut t = Terminal::new(10, 10, 0).unwrap();
    t.scrolling_region.left = 2;
    t.scrolling_region.right = 4;
    t.set_cursor_pos(1, 3);
    t.print_string("ABC").unwrap();
    t.set_cursor_pos(1, 3);
    clear_dirty(&mut t);
    t.insert_blanks(2);
    assert!(dirty(&t, 0));
    t.print('X').unwrap();
    assert_text(&t, "  X A");
}

#[test]
fn editing_insert_blanks_outside_left_right_scroll_region() {
    let mut t = Terminal::new(6, 10, 0).unwrap();
    t.set_cursor_pos(1, 4);
    t.print_string("ABC").unwrap();
    t.scrolling_region.left = 2;
    t.scrolling_region.right = 4;
    assert!(t.screens.active().cursor.pending_wrap);
    clear_dirty(&mut t);
    t.insert_blanks(2);
    assert!(!dirty(&t, 0));
    assert!(!t.screens.active().cursor.pending_wrap);
    t.print('X').unwrap();
    assert_text(&t, "   ABX");
}

#[test]
fn editing_insert_blanks_left_right_scroll_region_large_count() {
    let mut t = Terminal::new(10, 10, 0).unwrap();
    t.modes.set(Mode::Origin, true);
    t.modes.set(Mode::EnableLeftAndRightMargin, true);
    t.set_left_and_right_margin(3, 5);
    t.set_cursor_pos(1, 1);
    clear_dirty(&mut t);
    t.insert_blanks(140);
    assert!(dirty(&t, 0));
    t.print('X').unwrap();
    assert_text(&t, "  X");
}

fn grapheme_count(t: &Terminal) -> usize {
    let s = t.screens.active();
    let pin = s.pages.tracked_pin(s.cursor.page_pin).unwrap();
    s.pages.page(pin.node).unwrap().grapheme_count()
}

#[test]
fn editing_insert_blanks_deleting_graphemes() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.modes.set(Mode::GraphemeCluster, true);
    t.print_string("ABC👨‍👩‍👧").unwrap();
    assert_eq!(grapheme_count(&t), 1);
    t.set_cursor_pos(1, 1);
    clear_dirty(&mut t);
    t.insert_blanks(4);
    assert!(dirty(&t, 0));
    assert_text(&t, "    A");
    assert_eq!(grapheme_count(&t), 0);
}

#[test]
fn editing_insert_blanks_shift_graphemes() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.modes.set(Mode::GraphemeCluster, true);
    t.print_string("A👨‍👩‍👧").unwrap();
    assert_eq!(grapheme_count(&t), 1);
    t.set_cursor_pos(1, 1);
    clear_dirty(&mut t);
    t.insert_blanks(1);
    assert!(dirty(&t, 0));
    assert_text(&t, " A👨‍👩‍👧");
    assert_eq!(grapheme_count(&t), 1);
}

#[test]
fn editing_insert_blanks_split_multi_cell_character_from_tail() {
    let mut t = Terminal::new(5, 10, 0).unwrap();
    t.print_string("橋123").unwrap();
    t.set_cursor_pos(1, 2);
    t.insert_blanks(1);
    assert_text(&t, "   12");
}

#[test]
fn editing_insert_blanks_shifts_hyperlinks() {
    let mut t = Terminal::new(10, 2, 0).unwrap();
    t.screens
        .active_mut()
        .start_hyperlink(b"http://example.com", None)
        .unwrap();
    t.print_string("ABC").unwrap();
    t.set_cursor_pos(1, 1);
    t.insert_blanks(2);
    assert_text(&t, "  ABC");
    assert_links(&t, 0, &[false, false, true, true, true]);
}

#[test]
fn editing_insert_blanks_pushes_hyperlink_off_end_completely() {
    let mut t = Terminal::new(3, 2, 0).unwrap();
    t.screens
        .active_mut()
        .start_hyperlink(b"http://example.com", None)
        .unwrap();
    t.print_string("ABC").unwrap();
    t.set_cursor_pos(1, 1);
    t.insert_blanks(3);
    assert_text(&t, "");
    assert_links(&t, 0, &[false; 3]);
    assert!(!row(&t, 0).hyperlink());
}

#[test]
fn editing_insert_blanks_wide_char_straddling_right_margin() {
    let mut t = Terminal::new(10, 5, 0).unwrap();
    t.set_cursor_pos(1, 1);
    t.print_string("ABCD橋").unwrap();
    t.scrolling_region.right = 4;
    t.set_cursor_pos(1, 3);
    t.insert_blanks(1);
    assert_text(&t, "AB CD");
    assert_eq!(
        cell(&t, 5, 0).wide(),
        maestro_terminal::page::cell::Wide::Narrow
    );
}

#[test]
fn editing_insert_blanks_wide_char_spacer_tail_orphaned_beyond_right_margin() {
    let mut t = Terminal::new(10, 5, 0).unwrap();
    t.print_string("中中中中中").unwrap();
    t.modes.set(Mode::EnableLeftAndRightMargin, true);
    t.set_left_and_right_margin(1, 9);
    t.print('a').unwrap();
    t.insert_blanks(8);
    assert_text(&t, "a");
    for x in 1..10 {
        assert_eq!(
            cell(&t, x, 0).wide(),
            maestro_terminal::page::cell::Wide::Narrow
        );
    }
}

#[test]
fn editing_delete_lines_wide_char_at_right_margin_with_full_clear() {
    let mut t = Terminal::new(80, 24, 0).unwrap();
    t.set_cursor_pos(10, 39);
    t.print('中').unwrap();
    t.modes.set(Mode::EnableLeftAndRightMargin, true);
    t.set_left_and_right_margin(5, 39);
    t.scroll_up(usize::from(t.rows())).unwrap();
    assert_text(&t, "");
    assert_eq!(
        cell(&t, 39, 9).wide(),
        maestro_terminal::page::cell::Wide::Narrow
    );
}
