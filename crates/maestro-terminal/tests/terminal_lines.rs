//! Terminal tab destinations and line control.

use super::support::terminal::{assert_text, clear_dirty, dirty};
use maestro_terminal::{Mode, Terminal, csi::TabClear};

#[test]
fn terminal_clear_absent_tab_does_not_create_a_destination() {
    let mut terminal = Terminal::new(80, 24, 0).unwrap();
    terminal.set_cursor_pos(1, 4);
    terminal.tab_clear(TabClear::Current);
    terminal.set_cursor_pos(1, 1);
    terminal.horizontal_tab();
    assert_eq!(terminal.screens.active().cursor.x, 8);
}

#[test]
fn line_control_horizontal_tabs_back() {
    let mut terminal = Terminal::new(20, 5, 0).unwrap();
    terminal.set_cursor_pos(1, 20);
    terminal.horizontal_tab_back();
    assert_eq!(terminal.screens.active().cursor.x, 16);
    terminal.horizontal_tab_back();
    assert_eq!(terminal.screens.active().cursor.x, 8);
    terminal.horizontal_tab_back();
    assert_eq!(terminal.screens.active().cursor.x, 0);
    terminal.horizontal_tab_back();
    assert_eq!(terminal.screens.active().cursor.x, 0);
}

#[test]
fn line_control_tabclear_single() {
    let mut terminal = Terminal::new(30, 5, 0).unwrap();
    terminal.horizontal_tab();
    terminal.tab_clear(TabClear::Current);
    let screen = terminal.screens.active_mut();
    let pin = screen
        .pages
        .pin(maestro_terminal::Point::Active(
            maestro_terminal::Coordinate { x: 0, y: 0 },
        ))
        .unwrap();
    assert_eq!(pin.is_dirty(&screen.pages), Some(false));
    terminal.set_cursor_pos(1, 1);
    terminal.horizontal_tab();
    assert_eq!(terminal.screens.active().cursor.x, 16);
}

#[test]
fn line_control_tabclear_all() {
    let mut terminal = Terminal::new(30, 5, 0).unwrap();
    terminal.tab_clear(TabClear::All);
    let screen = terminal.screens.active_mut();
    let pin = screen
        .pages
        .pin(maestro_terminal::Point::Active(
            maestro_terminal::Coordinate { x: 0, y: 0 },
        ))
        .unwrap();
    assert_eq!(pin.is_dirty(&screen.pages), Some(false));
    terminal.set_cursor_pos(1, 1);
    terminal.horizontal_tab();
    assert_eq!(terminal.screens.active().cursor.x, 29);
}

#[test]
fn line_control_linefeed_and_carriage_return() {
    let mut t = Terminal::new(80, 80, 0).unwrap();

    t.print_string("hello").unwrap();
    clear_dirty(&mut t);
    t.carriage_return();

    assert!(!dirty(&t, 0));

    t.linefeed().unwrap();

    assert!(dirty(&t, 0));
    assert!(dirty(&t, 1));

    t.print_string("world").unwrap();
    assert_eq!(t.screens.active().cursor.y, 1);
    assert_eq!(t.screens.active().cursor.x, 5);
    assert_text(&t, "hello\nworld");
}

#[test]
fn line_control_linefeed_unsets_pending_wrap() {
    let mut t = Terminal::new(5, 80, 0).unwrap();

    t.print_string("hello").unwrap();
    assert!(t.screens.active().cursor.pending_wrap);
    clear_dirty(&mut t);
    t.linefeed().unwrap();
    assert!(dirty(&t, 0));
    assert!(dirty(&t, 1));
    assert!(!t.screens.active().cursor.pending_wrap);
}

#[test]
fn line_control_linefeed_mode_automatic_carriage_return() {
    let mut t = Terminal::new(10, 10, 0).unwrap();

    t.modes.set(Mode::Linefeed, true);
    t.print_string("123456").unwrap();
    t.linefeed().unwrap();
    t.print('X').unwrap();
    assert_text(&t, "123456\nX");
}

#[test]
fn line_control_carriage_return_unsets_pending_wrap() {
    let mut t = Terminal::new(5, 80, 0).unwrap();

    t.print_string("hello").unwrap();
    assert!(t.screens.active().cursor.pending_wrap);
    t.carriage_return();
    assert!(!t.screens.active().cursor.pending_wrap);
}

#[test]
fn line_control_carriage_return_origin_mode_moves_to_left_margin() {
    let mut t = Terminal::new(5, 80, 0).unwrap();

    t.modes.set(Mode::Origin, true);
    t.set_cursor_pos(1, 1);
    t.scrolling_region.left = 2;
    t.carriage_return();
    assert_eq!(t.screens.active().cursor.x, 2);
}

#[test]
fn line_control_carriage_return_left_of_left_margin_moves_to_zero() {
    let mut t = Terminal::new(5, 80, 0).unwrap();

    t.set_cursor_pos(1, 2);
    t.scrolling_region.left = 2;
    t.carriage_return();
    assert_eq!(t.screens.active().cursor.x, 0);
}

#[test]
fn line_control_carriage_return_right_of_left_margin_moves_to_left_margin() {
    let mut t = Terminal::new(5, 80, 0).unwrap();

    t.set_cursor_pos(1, 4);
    t.scrolling_region.left = 2;
    t.carriage_return();
    assert_eq!(t.screens.active().cursor.x, 2);
}

#[test]
fn line_control_backspace() {
    let mut t = Terminal::new(80, 80, 0).unwrap();

    t.print_string("hello").unwrap();
    t.backspace();
    t.print('y').unwrap();
    assert_eq!(t.screens.active().cursor.y, 0);
    assert_eq!(t.screens.active().cursor.x, 5);
    assert_text(&t, "helly");
}

#[test]
fn line_control_horizontal_tabs() {
    let mut t = Terminal::new(20, 5, 0).unwrap();

    t.print('1').unwrap();
    t.horizontal_tab();
    assert_eq!(t.screens.active().cursor.x, 8);

    t.horizontal_tab();
    assert_eq!(t.screens.active().cursor.x, 16);

    t.horizontal_tab();
    assert_eq!(t.screens.active().cursor.x, 19);
    t.horizontal_tab();
    assert_eq!(t.screens.active().cursor.x, 19);
}

#[test]
fn line_control_horizontal_tabs_starting_on_tabstop() {
    let mut t = Terminal::new(20, 5, 0).unwrap();

    t.set_cursor_pos(usize::from(t.screens.active().cursor.y), 9);
    t.print('X').unwrap();
    t.set_cursor_pos(usize::from(t.screens.active().cursor.y), 9);
    t.horizontal_tab();
    t.print('A').unwrap();

    assert_text(&t, "        X       A");
}

#[test]
fn line_control_horizontal_tabs_with_right_margin() {
    let mut t = Terminal::new(20, 5, 0).unwrap();

    t.scrolling_region.left = 2;
    t.scrolling_region.right = 5;
    t.set_cursor_pos(usize::from(t.screens.active().cursor.y), 1);
    t.print('X').unwrap();
    t.horizontal_tab();
    t.print('A').unwrap();

    assert_text(&t, "X    A");
}

#[test]
fn line_control_horizontal_tabs_back_starting_on_tabstop() {
    let mut t = Terminal::new(20, 5, 0).unwrap();

    t.set_cursor_pos(usize::from(t.screens.active().cursor.y), 9);
    t.print('X').unwrap();
    t.set_cursor_pos(usize::from(t.screens.active().cursor.y), 9);
    t.horizontal_tab_back();
    t.print('A').unwrap();

    assert_text(&t, "A       X");
}

#[test]
fn line_control_horizontal_tabs_with_left_margin_in_origin_mode() {
    let mut t = Terminal::new(20, 5, 0).unwrap();

    t.modes.set(Mode::Origin, true);
    t.scrolling_region.left = 2;
    t.scrolling_region.right = 5;
    t.set_cursor_pos(1, 2);
    t.print('X').unwrap();
    t.horizontal_tab_back();
    t.print('A').unwrap();

    assert_text(&t, "  AX");
}

#[test]
fn line_control_horizontal_tab_back_with_cursor_before_left_margin() {
    let mut t = Terminal::new(20, 5, 0).unwrap();

    t.modes.set(Mode::Origin, true);
    t.save_cursor();
    t.modes.set(Mode::EnableLeftAndRightMargin, true);
    t.scrolling_region.left = 4;
    t.scrolling_region.right = 19;
    t.set_cursor_pos(1, 1);
    t.restore_cursor();
    t.horizontal_tab_back();
    t.print('X').unwrap();

    assert_text(&t, "X");
}

#[test]
fn line_control_reverseindex() {
    let mut t = Terminal::new(2, 5, 0).unwrap();

    t.print('A').unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print('B').unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print('C').unwrap();
    t.reverse_index().unwrap();
    t.print('D').unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.carriage_return();
    t.linefeed().unwrap();

    assert_text(&t, "A\nBD\nC");
}

#[test]
fn line_control_reverseindex_from_the_top() {
    let mut t = Terminal::new(2, 5, 0).unwrap();

    t.print('A').unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print('B').unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.carriage_return();
    t.linefeed().unwrap();

    t.set_cursor_pos(1, 1);
    t.reverse_index().unwrap();
    t.print('D').unwrap();

    t.carriage_return();
    t.linefeed().unwrap();
    t.set_cursor_pos(1, 1);
    t.reverse_index().unwrap();
    t.print('E').unwrap();
    t.carriage_return();
    t.linefeed().unwrap();

    assert_text(&t, "E\nD\nA\nB");
}

#[test]
fn line_control_reverseindex_top_of_scrolling_region() {
    let mut t = Terminal::new(2, 10, 0).unwrap();

    t.set_cursor_pos(2, 1);
    t.print('A').unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print('B').unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print('C').unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print('D').unwrap();
    t.carriage_return();
    t.linefeed().unwrap();

    t.scrolling_region.top = 1;
    t.scrolling_region.bottom = 4;
    t.set_cursor_pos(1, 1);
    t.set_cursor_pos(2, 1);
    t.reverse_index().unwrap();
    t.print('X').unwrap();

    assert_text(&t, "\nX\nA\nB\nC");
}

#[test]
fn line_control_reverseindex_top_bottom_margins() {
    let mut t = Terminal::new(5, 5, 0).unwrap();

    t.print('A').unwrap();
    t.set_cursor_pos(2, 1);
    t.print('B').unwrap();
    t.set_cursor_pos(3, 1);
    t.print('C').unwrap();
    t.scrolling_region.top = 1;
    t.scrolling_region.bottom = 2;
    t.set_cursor_pos(1, 1);
    t.set_cursor_pos(2, 1);
    t.reverse_index().unwrap();

    assert_text(&t, "A\n\nB");
}

#[test]
fn line_control_reverseindex_outside_top_bottom_margins() {
    let mut t = Terminal::new(5, 5, 0).unwrap();

    t.print('A').unwrap();
    t.set_cursor_pos(2, 1);
    t.print('B').unwrap();
    t.set_cursor_pos(3, 1);
    t.print('C').unwrap();
    t.scrolling_region.top = 1;
    t.scrolling_region.bottom = 2;
    t.set_cursor_pos(1, 1);
    t.set_cursor_pos(1, 1);
    t.reverse_index().unwrap();

    assert_text(&t, "A\nB\nC");
}

#[test]
fn line_control_reverseindex_left_right_margins() {
    let mut t = Terminal::new(5, 5, 0).unwrap();

    t.print_string("ABC").unwrap();
    t.set_cursor_pos(2, 1);
    t.print_string("DEF").unwrap();
    t.set_cursor_pos(3, 1);
    t.print_string("GHI").unwrap();
    t.modes.set(Mode::EnableLeftAndRightMargin, true);
    t.scrolling_region.left = 1;
    t.scrolling_region.right = 2;
    t.set_cursor_pos(1, 1);
    t.set_cursor_pos(1, 2);
    t.reverse_index().unwrap();

    assert_text(&t, "A\nDBC\nGEF\n HI");
}

#[test]
fn line_control_reverseindex_outside_left_right_margins() {
    let mut t = Terminal::new(5, 5, 0).unwrap();

    t.print_string("ABC").unwrap();
    t.set_cursor_pos(2, 1);
    t.print_string("DEF").unwrap();
    t.set_cursor_pos(3, 1);
    t.print_string("GHI").unwrap();
    t.modes.set(Mode::EnableLeftAndRightMargin, true);
    t.scrolling_region.left = 1;
    t.scrolling_region.right = 2;
    t.set_cursor_pos(1, 1);
    t.set_cursor_pos(1, 1);
    t.reverse_index().unwrap();

    assert_text(&t, "ABC\nDEF\nGHI");
}

#[test]
fn line_control_index() {
    let mut t = Terminal::new(2, 5, 0).unwrap();

    t.index().unwrap();
    t.print('A').unwrap();

    assert!(dirty(&t, 0));
    assert!(dirty(&t, 1));

    assert_text(&t, "\nA");
}

#[test]
fn line_control_index_from_the_bottom() {
    let mut t = Terminal::new(2, 5, 0).unwrap();

    t.set_cursor_pos(5, 1);
    t.print('A').unwrap();
    t.cursor_left(1);

    clear_dirty(&mut t);
    t.index().unwrap();
    t.print('B').unwrap();

    assert!(dirty(&t, 3));
    assert!(dirty(&t, 4));

    assert_text(&t, "\n\n\nA\nB");
}

#[test]
fn line_control_index_outside_of_scrolling_region() {
    let mut t = Terminal::new(2, 5, 0).unwrap();

    assert_eq!(t.screens.active().cursor.y, 0);
    t.scrolling_region.top = 1;
    t.scrolling_region.bottom = 4;
    t.set_cursor_pos(1, 1);
    t.index().unwrap();
    assert_eq!(t.screens.active().cursor.y, 1);
}

#[test]
fn line_control_index_from_the_bottom_outside_of_scroll_region() {
    let mut t = Terminal::new(2, 5, 0).unwrap();

    t.scrolling_region.top = 0;
    t.scrolling_region.bottom = 1;
    t.set_cursor_pos(1, 1);
    t.set_cursor_pos(5, 1);
    t.print('A').unwrap();
    clear_dirty(&mut t);
    t.index().unwrap();
    t.print('B').unwrap();
    assert!(dirty(&t, 4));

    assert_text(&t, "\n\n\n\nAB");
}

#[test]
fn line_control_index_inside_scroll_region() {
    let mut t = Terminal::new(5, 5, 0).unwrap();

    t.scrolling_region.top = 0;
    t.scrolling_region.bottom = 2;
    t.set_cursor_pos(1, 1);
    t.print('A').unwrap();
    clear_dirty(&mut t);
    t.index().unwrap();
    t.print('X').unwrap();

    assert!(dirty(&t, 0));
    assert!(dirty(&t, 1));

    assert_text(&t, "A\n X");
}

#[test]
fn line_control_index_outside_left_right_margin() {
    let mut t = Terminal::new(10, 5, 0).unwrap();

    t.scrolling_region.top = 0;
    t.scrolling_region.bottom = 2;
    t.set_cursor_pos(1, 1);
    t.scrolling_region.left = 3;
    t.scrolling_region.right = 5;
    t.set_cursor_pos(3, 3);
    t.print('A').unwrap();
    t.set_cursor_pos(3, 1);
    clear_dirty(&mut t);
    t.index().unwrap();
    t.print('X').unwrap();

    assert!(dirty(&t, 2));

    assert_text(&t, "\n\nX A");
}

#[test]
fn line_control_index_inside_left_right_margin() {
    let mut t = Terminal::new(10, 5, 0).unwrap();

    t.print_string("AAAAAA").unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print_string("AAAAAA").unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print_string("AAAAAA").unwrap();
    t.modes.set(Mode::EnableLeftAndRightMargin, true);
    t.scrolling_region.top = 0;
    t.scrolling_region.bottom = 2;
    t.set_cursor_pos(1, 1);
    t.scrolling_region.left = 0;
    t.scrolling_region.right = 2;
    t.set_cursor_pos(1, 1);
    t.set_cursor_pos(3, 1);

    clear_dirty(&mut t);
    t.index().unwrap();

    assert!(dirty(&t, 0));
    assert!(dirty(&t, 1));
    assert!(dirty(&t, 2));

    assert_eq!(t.screens.active().cursor.y, 2);
    assert_eq!(t.screens.active().cursor.x, 0);

    assert_text(&t, "AAAAAA\nAAAAAA\n   AAA");
}

#[test]
fn line_control_index_bottom_of_scroll_region_no_scrollback() {
    let mut t = Terminal::new(5, 5, 0).unwrap();

    t.scrolling_region.top = 0;
    t.scrolling_region.bottom = 2;
    t.set_cursor_pos(1, 1);
    t.set_cursor_pos(4, 1);
    t.print('B').unwrap();
    t.set_cursor_pos(3, 1);
    t.print('A').unwrap();
    clear_dirty(&mut t);
    t.index().unwrap();
    t.print('X').unwrap();

    assert_text(&t, "\nA\n X\nB");
}

fn assert_link(t: &Terminal, y: u32, expected: Option<u16>) {
    use maestro_terminal::{Coordinate, Point, page::CellPosition};
    let screen = t.screens.active();
    let pin = screen
        .pages
        .pin(Point::Viewport(Coordinate { x: 0, y }))
        .unwrap();
    let (row, cell) = pin.row_and_cell(&screen.pages).unwrap();
    let page = screen.pages.page(pin.node).unwrap();
    assert_eq!(
        (row.hyperlink(), cell.hyperlink()),
        (expected.is_some(), expected.is_some())
    );
    assert_eq!(
        page.lookup_hyperlink(CellPosition { x: 0, y: pin.y }),
        expected
    );
}

#[test]
fn line_control_index_scrolling_with_hyperlink() {
    let mut t = Terminal::new(2, 5, 0).unwrap();
    t.set_cursor_pos(5, 1);
    t.screens
        .active_mut()
        .start_hyperlink(b"http://example.com", None)
        .unwrap();
    t.print('A').unwrap();
    t.screens.active_mut().end_hyperlink();
    t.cursor_left(1);
    t.index().unwrap();
    t.print('B').unwrap();
    assert_text(&t, "\n\n\nA\nB");
    assert_link(&t, 3, Some(1));
    assert_link(&t, 4, None);
}

#[test]
fn line_control_index_bottom_of_scroll_region_with_hyperlinks() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.scrolling_region.bottom = 1;
    t.set_cursor_pos(1, 1);
    t.print('A').unwrap();
    t.index().unwrap();
    t.carriage_return();
    t.screens
        .active_mut()
        .start_hyperlink(b"http://example.com", None)
        .unwrap();
    t.print('B').unwrap();
    t.screens.active_mut().end_hyperlink();
    t.index().unwrap();
    t.carriage_return();
    t.print('C').unwrap();
    assert_text(&t, "B\nC");
    assert_link(&t, 0, Some(1));
    assert_link(&t, 1, None);
}

fn assert_red_background(t: &Terminal, y: u32) {
    use maestro_terminal::{color::Rgb, page::cell::Content};
    for x in 0..t.cols() {
        assert_eq!(
            super::support::terminal::cell(t, x, y).content(),
            Content::BgColorRgb(Rgb { r: 255, g: 0, b: 0 })
        );
    }
}

#[test]
fn line_control_index_bottom_of_scroll_region_with_background_sgr() {
    use maestro_terminal::{color::Rgb, sgr::Attribute};
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.scrolling_region.bottom = 2;
    t.set_cursor_pos(4, 1);
    t.print('B').unwrap();
    t.set_cursor_pos(3, 1);
    t.print('A').unwrap();
    t.set_attribute(Attribute::DirectColorBg(Rgb { r: 255, g: 0, b: 0 }))
        .unwrap();
    t.index().unwrap();
    assert_text(&t, "\nA\n\nB");
    assert_red_background(&t, 2);
}

fn assert_history_one(t: &Terminal) {
    use maestro_terminal::{Coordinate, Point};
    let screen = t.screens.active();
    assert_eq!(t.total_rows(), 6);
    for x in 0..t.cols() {
        let pin = screen
            .pages
            .pin(Point::History(Coordinate { x, y: 0 }))
            .unwrap();
        let cp = pin.row_and_cell(&screen.pages).unwrap().1.codepoint();
        assert_eq!(cp, if x == 0 { '1' } else { '\0' });
    }
}

#[test]
fn line_control_index_bottom_of_scroll_region_creates_scrollback() {
    let mut t = Terminal::new(5, 5, usize::MAX).unwrap();
    t.scrolling_region.bottom = 2;
    t.print_string("1\n2\n3").unwrap();
    t.set_cursor_pos(4, 1);
    t.print('X').unwrap();
    t.set_cursor_pos(3, 1);
    t.index().unwrap();
    t.print('Y').unwrap();
    assert_text(&t, "2\n3\nY\nX");
    assert_history_one(&t);
}

#[test]
fn line_control_index_bottom_of_scroll_region_blank_line_preserves_sgr() {
    use maestro_terminal::{color::Rgb, sgr::Attribute};
    let mut t = Terminal::new(5, 5, usize::MAX).unwrap();
    t.scrolling_region.bottom = 2;
    t.print_string("1\n2\n3").unwrap();
    t.set_cursor_pos(4, 1);
    t.print('X').unwrap();
    t.set_cursor_pos(3, 1);
    t.set_attribute(Attribute::DirectColorBg(Rgb { r: 255, g: 0, b: 0 }))
        .unwrap();
    t.index().unwrap();
    assert_text(&t, "2\n3\n\nX");
    assert_history_one(&t);
    assert_red_background(&t, 2);
}

#[test]
fn line_control_index_bottom_of_scroll_region_with_top_margin_and_background_sgr() {
    use maestro_terminal::{color::Rgb, sgr::Attribute};
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.print_string("1\n2\n3\n4\n5").unwrap();
    t.scrolling_region.top = 1;
    t.scrolling_region.bottom = 3;
    t.set_cursor_pos(4, 1);
    t.set_attribute(Attribute::DirectColorBg(Rgb { r: 255, g: 0, b: 0 }))
        .unwrap();
    t.index().unwrap();
    assert_text(&t, "1\n3\n4\n\n5");
    assert_eq!(t.screens.active().cursor.y, 3);
    assert_red_background(&t, 3);
}
