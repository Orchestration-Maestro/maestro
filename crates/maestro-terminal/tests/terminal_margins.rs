//! Margin requests observed through cursor movement and row editing.

use super::support::terminal::{assert_text, clear_dirty, dirty};
use maestro_terminal::{Mode, Terminal};

#[test]
fn margin_requests_home_in_origin_mode_and_preserve_rejected_state() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.modes.set(Mode::Origin, true);
    t.set_top_and_bottom_margin(2, usize::MAX);
    assert_eq!((t.scrolling_region.top, t.scrolling_region.bottom), (1, 4));
    assert_eq!(
        (t.screens.active().cursor.x, t.screens.active().cursor.y),
        (0, 1)
    );
    t.modes.set(Mode::EnableLeftAndRightMargin, true);
    t.set_left_and_right_margin(3, usize::MAX);
    assert_eq!((t.scrolling_region.left, t.scrolling_region.right), (2, 4));
    assert_eq!(
        (t.screens.active().cursor.x, t.screens.active().cursor.y),
        (2, 1)
    );
    t.set_cursor_pos(3, 3);
    t.print('X').unwrap();
    assert!(t.screens.active().cursor.pending_wrap);
    clear_dirty(&mut t);
    let region = t.scrolling_region;
    for (start, end) in [(4, 4), (usize::MAX, 0), (5, 2)] {
        t.set_top_and_bottom_margin(start, end);
        t.set_left_and_right_margin(start, end);
        assert_eq!(t.scrolling_region, region);
        assert_eq!(
            (t.screens.active().cursor.x, t.screens.active().cursor.y),
            (4, 3)
        );
        assert!(t.screens.active().cursor.pending_wrap);
        assert!((0..5).all(|y| !dirty(&t, y)));
    }
    t.modes.set(Mode::EnableLeftAndRightMargin, false);
    t.set_left_and_right_margin(0, 0);
    assert_eq!(t.scrolling_region, region);
    assert!(t.screens.active().cursor.pending_wrap);
    assert!((0..5).all(|y| !dirty(&t, y)));
}

#[test]
fn editing_set_top_and_bottom_margin_simple() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.print_string("ABC").unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print_string("DEF").unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print_string("GHI").unwrap();
    t.set_top_and_bottom_margin(0, 0);
    clear_dirty(&mut t);
    t.scroll_down(1).unwrap();
    for y in 0..4 {
        assert!(dirty(&t, y));
    }
    assert_text(&t, "\nABC\nDEF\nGHI");
}

#[test]
fn editing_set_left_and_right_margin_left_only() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.print_string("ABC").unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print_string("DEF").unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print_string("GHI").unwrap();
    t.modes.set(Mode::EnableLeftAndRightMargin, true);
    t.set_left_and_right_margin(2, 0);
    assert_eq!(t.scrolling_region.left, 1);
    assert_eq!(t.scrolling_region.right, t.cols() - 1);
    t.set_cursor_pos(1, 2);
    clear_dirty(&mut t);
    t.insert_lines(1).unwrap();
    for y in 0..4 {
        assert!(dirty(&t, y));
    }
    assert_text(&t, "A\nDBC\nGEF\n HI");
}

#[test]
fn editing_set_top_and_bottom_margin_top_only() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.print_string("ABC").unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print_string("DEF").unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print_string("GHI").unwrap();
    t.set_top_and_bottom_margin(2, 0);
    clear_dirty(&mut t);
    t.scroll_down(1).unwrap();
    assert!(dirty(&t, 0));
    assert!(dirty(&t, 1));
    assert!(dirty(&t, 2));
    assert!(dirty(&t, 3));
    assert_text(&t, "ABC\n\nDEF\nGHI");
}

#[test]
fn editing_set_top_and_bottom_margin_top_and_bottom() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.print_string("ABC").unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print_string("DEF").unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print_string("GHI").unwrap();
    t.set_top_and_bottom_margin(1, 2);
    clear_dirty(&mut t);
    t.scroll_down(1).unwrap();
    assert!(dirty(&t, 0));
    assert!(dirty(&t, 1));
    assert!(!dirty(&t, 2));
    assert_text(&t, "\nABC\nGHI");
}

#[test]
fn editing_set_top_and_bottom_margin_top_equal_to_bottom() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.print_string("ABC").unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print_string("DEF").unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print_string("GHI").unwrap();
    t.set_top_and_bottom_margin(2, 2);
    clear_dirty(&mut t);
    t.scroll_down(1).unwrap();
    assert!(dirty(&t, 0));
    assert!(dirty(&t, 1));
    assert!(dirty(&t, 2));
    assert!(dirty(&t, 3));
    assert_text(&t, "\nABC\nDEF\nGHI");
}

#[test]
fn editing_set_left_and_right_margin_left_and_right() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.print_string("ABC").unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print_string("DEF").unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print_string("GHI").unwrap();
    t.modes.set(Mode::EnableLeftAndRightMargin, true);
    t.set_left_and_right_margin(1, 2);
    t.set_cursor_pos(1, 2);
    clear_dirty(&mut t);
    t.insert_lines(1).unwrap();
    assert!(dirty(&t, 0));
    assert!(dirty(&t, 1));
    assert!(dirty(&t, 2));
    assert!(dirty(&t, 3));
    assert_text(&t, "  C\nABF\nDEI\nGH");
}

#[test]
fn editing_set_left_and_right_margin_left_equal_right() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.print_string("ABC").unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print_string("DEF").unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print_string("GHI").unwrap();
    t.modes.set(Mode::EnableLeftAndRightMargin, true);
    t.set_left_and_right_margin(2, 2);
    t.set_cursor_pos(1, 2);
    clear_dirty(&mut t);
    t.insert_lines(1).unwrap();
    assert!(dirty(&t, 0));
    assert!(dirty(&t, 1));
    assert!(dirty(&t, 2));
    assert!(dirty(&t, 3));
    assert_text(&t, "\nABC\nDEF\nGHI");
}

#[test]
fn editing_set_left_and_right_margin_mode_69_unset() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.print_string("ABC").unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print_string("DEF").unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print_string("GHI").unwrap();
    t.modes.set(Mode::EnableLeftAndRightMargin, false);
    t.set_left_and_right_margin(1, 2);
    t.set_cursor_pos(1, 2);
    clear_dirty(&mut t);
    t.insert_lines(1).unwrap();
    assert!(dirty(&t, 0));
    assert!(dirty(&t, 1));
    assert!(dirty(&t, 2));
    assert!(dirty(&t, 3));
    assert_text(&t, "\nABC\nDEF\nGHI");
}

#[test]
fn editing_set_left_and_right_margin_simple() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.print_string("ABC\nDEF\nGHI").unwrap();
    t.modes.set(Mode::EnableLeftAndRightMargin, true);
    t.set_left_and_right_margin(0, 0);
    clear_dirty(&mut t);
    t.erase_chars(1);
    assert!(dirty(&t, 0));
    assert!(!dirty(&t, 1));
    assert_text(&t, " BC\nDEF\nGHI");
}

#[test]
fn editing_cursor_pos_off_the_screen() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.set_cursor_pos(500, 500);
    t.print('X').unwrap();
    assert_text(&t, "\n\n\n\n    X");
}

#[test]
fn editing_cursor_right_to_the_edge_of_screen() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.cursor_right(100);
    t.print('X').unwrap();
    assert_text(&t, "    X");
}
