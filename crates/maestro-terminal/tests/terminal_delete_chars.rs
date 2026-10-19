//! Character deletion through the direct terminal interface.

use super::support::terminal::{assert_text, cell, clear_dirty, dirty, row};
use maestro_terminal::{
    Mode, Terminal,
    color::Rgb,
    page::cell::{Content, Wide},
    sgr::Attribute,
};

#[test]
fn editing_delete_chars() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.print_string("ABCDE").unwrap();
    t.set_cursor_pos(1, 2);
    clear_dirty(&mut t);
    t.delete_chars(2);
    assert!(dirty(&t, 0));
    assert_text(&t, "ADE");
}

#[test]
fn editing_delete_chars_zero_count() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.print_string("ABCDE").unwrap();
    t.set_cursor_pos(1, 2);
    clear_dirty(&mut t);
    t.delete_chars(0);
    assert!(!dirty(&t, 0));
    assert_text(&t, "ABCDE");
}

#[test]
fn editing_delete_chars_more_than_half() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.print_string("ABCDE").unwrap();
    t.set_cursor_pos(1, 2);
    clear_dirty(&mut t);
    t.delete_chars(3);
    assert!(dirty(&t, 0));
    assert_text(&t, "AE");
}

#[test]
fn editing_delete_chars_more_than_line_width() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.print_string("ABCDE").unwrap();
    t.set_cursor_pos(1, 2);
    clear_dirty(&mut t);
    t.delete_chars(10);
    assert!(dirty(&t, 0));
    assert_text(&t, "A");
}

#[test]
fn editing_delete_chars_should_shift_left() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.print_string("ABCDE").unwrap();
    t.set_cursor_pos(1, 2);
    clear_dirty(&mut t);
    t.delete_chars(1);
    assert!(dirty(&t, 0));
    assert_text(&t, "ACDE");
}

#[test]
fn editing_delete_chars_resets_pending_wrap() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.print_string("ABCDE").unwrap();
    assert!(t.screens.active().cursor.pending_wrap);
    t.delete_chars(1);
    assert!(!t.screens.active().cursor.pending_wrap);
    t.print('X').unwrap();
    assert_text(&t, "ABCDX");
}

#[test]
fn editing_delete_chars_resets_wrap() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.print_string("ABCDE123").unwrap();
    assert!(row(&t, 0).wrap());
    t.set_cursor_pos(1, 1);
    t.delete_chars(1);
    assert!(!row(&t, 0).wrap());
    t.print('X').unwrap();
    assert_text(&t, "XCDE\n123");
}

#[test]
fn editing_delete_chars_simple_operation() {
    let mut t = Terminal::new(10, 10, 0).unwrap();
    t.print_string("ABC123").unwrap();
    t.set_cursor_pos(1, 3);
    clear_dirty(&mut t);
    t.delete_chars(2);
    assert!(dirty(&t, 0));
    assert_text(&t, "AB23");
}

#[test]
fn editing_delete_chars_preserves_background_sgr() {
    let mut t = Terminal::new(10, 10, 0).unwrap();
    t.print_string("ABC123").unwrap();
    t.set_cursor_pos(1, 3);
    let red = Rgb { r: 255, g: 0, b: 0 };
    t.set_attribute(Attribute::DirectColorBg(red)).unwrap();
    t.delete_chars(2);
    assert_text(&t, "AB23");
    for x in t.cols() - 2..t.cols() {
        assert_eq!(cell(&t, x, 0).content(), Content::BgColorRgb(red));
    }
}

#[test]
fn editing_delete_chars_outside_scroll_region() {
    let mut t = Terminal::new(6, 10, 0).unwrap();
    t.print_string("ABC123").unwrap();
    t.scrolling_region.left = 2;
    t.scrolling_region.right = 4;
    assert!(t.screens.active().cursor.pending_wrap);
    clear_dirty(&mut t);
    t.delete_chars(2);
    assert!(!dirty(&t, 0));
    assert!(t.screens.active().cursor.pending_wrap);
    assert_text(&t, "ABC123");
}

#[test]
fn editing_delete_chars_inside_scroll_region() {
    let mut t = Terminal::new(6, 10, 0).unwrap();
    t.print_string("ABC123").unwrap();
    t.scrolling_region.left = 2;
    t.scrolling_region.right = 4;
    t.set_cursor_pos(1, 4);
    clear_dirty(&mut t);
    t.delete_chars(1);
    assert!(dirty(&t, 0));
    assert_text(&t, "ABC2 3");
}

#[test]
fn editing_delete_chars_split_wide_character_from_spacer_tail() {
    let mut t = Terminal::new(6, 10, 0).unwrap();
    t.print_string("A橋123").unwrap();
    t.set_cursor_pos(1, 3);
    t.delete_chars(1);
    assert_text(&t, "A 123");
}

#[test]
fn editing_delete_chars_split_wide_character_from_wide() {
    let mut t = Terminal::new(6, 10, 0).unwrap();
    t.print_string("橋123").unwrap();
    t.set_cursor_pos(1, 1);
    t.delete_chars(1);
    assert_eq!(cell(&t, 0, 0).codepoint(), '\0');
    assert_eq!(cell(&t, 0, 0).wide(), Wide::Narrow);
    assert_eq!(cell(&t, 1, 0).codepoint(), '1');
    assert_eq!(cell(&t, 1, 0).wide(), Wide::Narrow);
}

#[test]
fn editing_delete_chars_split_wide_character_from_end() {
    let mut t = Terminal::new(6, 10, 0).unwrap();
    t.print_string("A橋123").unwrap();
    t.set_cursor_pos(1, 1);
    t.delete_chars(1);
    assert_eq!(cell(&t, 0, 0).codepoint(), '橋');
    assert_eq!(cell(&t, 0, 0).wide(), Wide::Wide);
    assert_eq!(cell(&t, 1, 0).codepoint(), '\0');
    assert_eq!(cell(&t, 1, 0).wide(), Wide::SpacerTail);
}

#[test]
fn editing_delete_chars_with_a_spacer_head_at_the_end() {
    let mut t = Terminal::new(5, 10, 0).unwrap();
    t.print_string("0123橋123").unwrap();
    assert_eq!(cell(&t, 4, 0).wide(), Wide::SpacerHead);
    assert!(row(&t, 0).wrap());
    t.set_cursor_pos(1, 1);
    t.delete_chars(1);
    assert_eq!(cell(&t, 3, 0).codepoint(), '\0');
    assert_eq!(cell(&t, 3, 0).wide(), Wide::Narrow);
}

#[test]
fn editing_delete_chars_split_wide_character_tail() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.set_cursor_pos(1, usize::from(t.cols() - 1));
    t.print('橋').unwrap();
    t.carriage_return();
    t.delete_chars(usize::from(t.cols() - 1));
    t.print('0').unwrap();
    assert_text(&t, "0");
}

#[test]
fn editing_delete_chars_wide_char_boundary_conditions() {
    let mut t = Terminal::new(8, 1, 0).unwrap();
    t.print_string("😀a😀b😀").unwrap();
    assert_text(&t, "😀a😀b😀");
    t.set_cursor_pos(1, 2);
    t.delete_chars(3);
    assert_text(&t, "  b😀");
}

#[test]
fn editing_delete_chars_wide_char_wrap_boundary_conditions() {
    let mut t = Terminal::new(8, 3, 0).unwrap();
    t.print_string(".......😀abcde😀......").unwrap();
    assert_text(&t, ".......\n😀abcde\n😀......");
    assert!(row(&t, 0).wrap());
    assert!(row(&t, 1).wrap());
    t.set_cursor_pos(2, 2);
    t.delete_chars(3);
    assert_text(&t, ".......\n cde\n😀......");
    assert!(row(&t, 0).wrap());
    assert!(!row(&t, 1).wrap());
}

#[test]
fn editing_delete_chars_wide_char_across_right_margin() {
    let mut t = Terminal::new(8, 3, 0).unwrap();
    t.print_string("123456橋").unwrap();
    t.modes.set(Mode::EnableLeftAndRightMargin, true);
    t.set_left_and_right_margin(2, 7);
    assert_text(&t, "123456橋");
    t.set_cursor_pos(1, 2);
    t.delete_chars(1);
    assert_text(&t, "13456");
}
