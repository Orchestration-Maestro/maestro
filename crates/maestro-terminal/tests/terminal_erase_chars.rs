//! Character protection and erase behavior through direct terminal operations.

use maestro_terminal::{Terminal, ansi::ProtectedMode};

#[test]
fn editing_set_protected_mode() {
    let mut t = Terminal::new(3, 3, 0).unwrap();
    assert!(!t.screens.active().cursor.protected);
    t.set_protected_mode(ProtectedMode::Off);
    assert!(!t.screens.active().cursor.protected);
    t.set_protected_mode(ProtectedMode::Iso);
    assert!(t.screens.active().cursor.protected);
    assert_eq!(t.screens.active().protected_mode, ProtectedMode::Iso);
    t.set_protected_mode(ProtectedMode::Dec);
    assert!(t.screens.active().cursor.protected);
    assert_eq!(t.screens.active().protected_mode, ProtectedMode::Dec);
    t.set_protected_mode(ProtectedMode::Off);
    assert!(!t.screens.active().cursor.protected);
    assert_eq!(t.screens.active().protected_mode, ProtectedMode::Dec);
    t.set_protected_mode(ProtectedMode::Iso);
    t.set_protected_mode(ProtectedMode::Off);
    assert_eq!(t.screens.active().protected_mode, ProtectedMode::Iso);
}

#[test]
fn editing_erase_chars_simple_operation() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.print_string("ABC").unwrap();
    t.set_cursor_pos(1, 1);
    clear_dirty(&mut t);
    t.erase_chars(2);
    t.print('X').unwrap();
    assert_text(&t, "X C");
    assert!(dirty(&t, 0));
    assert!(!dirty(&t, 1));
}

use super::support::terminal::{assert_text, cell, clear_dirty, dirty, row};
use maestro_terminal::{page::cell::Content, sgr::Attribute};

#[test]
fn editing_erase_chars_minimum_one() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.print_string("ABC").unwrap();
    t.set_cursor_pos(1, 1);
    clear_dirty(&mut t);
    t.erase_chars(0);
    t.print('X').unwrap();
    assert!(dirty(&t, 0));
    assert_text(&t, "XBC");
}

#[test]
fn editing_erase_chars_beyond_screen_edge() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.print_string("  ABC").unwrap();
    t.set_cursor_pos(1, 4);
    t.erase_chars(10);
    assert_text(&t, "  A");
}

#[test]
fn editing_erase_chars_wide_character() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.print_string("橋BC").unwrap();
    t.set_cursor_pos(1, 1);
    t.erase_chars(1);
    t.print('X').unwrap();
    assert_text(&t, "X BC");
}

#[test]
fn editing_erase_chars_resets_pending_wrap() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.print_string("ABCDE").unwrap();
    assert!(t.screens.active().cursor.pending_wrap);
    t.erase_chars(1);
    assert!(!t.screens.active().cursor.pending_wrap);
    t.print('X').unwrap();
    assert_text(&t, "ABCDX");
}

#[test]
fn editing_erase_chars_resets_wrap() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.print_string("ABCDE123").unwrap();
    assert!(row(&t, 0).wrap());
    t.set_cursor_pos(1, 1);
    t.erase_chars(1);
    assert!(!row(&t, 0).wrap());
    t.print('X').unwrap();
    assert_text(&t, "XBCDE\n123");
}

#[test]
fn editing_erase_chars_preserves_background_sgr() {
    let mut t = Terminal::new(10, 10, 0).unwrap();
    t.print_string("ABC").unwrap();
    t.set_cursor_pos(1, 1);
    let rgb = maestro_terminal::color::Rgb { r: 255, g: 0, b: 0 };
    t.screens
        .active_mut()
        .set_attribute(Attribute::DirectColorBg(rgb))
        .unwrap();
    t.erase_chars(2);
    assert_text(&t, "  C");
    for x in 0..2 {
        assert_eq!(cell(&t, x, 0).content(), Content::BgColorRgb(rgb));
    }
}

#[test]
fn editing_erase_chars_handles_refcounted_styles() {
    let mut t = Terminal::new(10, 10, 0).unwrap();
    t.screens
        .active_mut()
        .set_attribute(Attribute::Bold)
        .unwrap();
    t.print_string("AB").unwrap();
    t.screens
        .active_mut()
        .set_attribute(Attribute::Unset)
        .unwrap();
    t.print('C').unwrap();
    assert_ne!(cell(&t, 0, 0).style_id(), 0);
    assert_ne!(cell(&t, 1, 0).style_id(), 0);
    t.set_cursor_pos(1, 1);
    t.erase_chars(2);
    assert_eq!(cell(&t, 0, 0).style_id(), 0);
    assert_eq!(cell(&t, 1, 0).style_id(), 0);
    assert_text(&t, "  C");
}

#[test]
fn editing_erase_chars_protected_attributes_respected_with_iso() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.set_protected_mode(ProtectedMode::Iso);
    t.print_string("ABC").unwrap();
    t.set_cursor_pos(1, 1);
    t.erase_chars(2);
    assert_text(&t, "ABC");
}

#[test]
fn editing_erase_chars_protected_attributes_ignored_with_dec_most_recent() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.set_protected_mode(ProtectedMode::Iso);
    t.print_string("ABC").unwrap();
    t.set_protected_mode(ProtectedMode::Dec);
    t.set_protected_mode(ProtectedMode::Off);
    t.set_cursor_pos(1, 1);
    t.erase_chars(2);
    assert_text(&t, "  C");
}

#[test]
fn editing_erase_chars_protected_attributes_ignored_with_dec_set() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.set_protected_mode(ProtectedMode::Dec);
    t.print_string("ABC").unwrap();
    t.set_cursor_pos(1, 1);
    t.erase_chars(2);
    assert_text(&t, "  C");
}

#[test]
fn editing_erase_chars_wide_char_boundary_conditions() {
    let mut t = Terminal::new(8, 1, 0).unwrap();
    t.print_string("😀a😀b😀").unwrap();
    assert_text(&t, "😀a😀b😀");
    t.set_cursor_pos(1, 2);
    t.erase_chars(3);
    assert_text(&t, "     b😀");
}

#[test]
fn editing_erase_chars_wide_char_splits_proper_cell_boundaries() {
    let mut t = Terminal::new(30, 1, 0).unwrap();
    t.print_string("x食べて下さい").unwrap();
    assert_text(&t, "x食べて下さい");
    t.set_cursor_pos(1, 6);
    t.erase_chars(4);
    assert_text(&t, "x食べ    さい");
}

#[test]
fn editing_erase_chars_wide_char_wrap_boundary_conditions() {
    let mut t = Terminal::new(8, 3, 0).unwrap();
    t.print_string(".......😀abcde😀......").unwrap();
    assert_text(&t, ".......\n😀abcde\n😀......");
    assert!(row(&t, 0).wrap());
    assert!(row(&t, 1).wrap());
    t.set_cursor_pos(2, 2);
    t.erase_chars(3);
    assert_text(&t, ".......\n    cde\n😀......");
    assert!(row(&t, 0).wrap());
    assert!(!row(&t, 1).wrap());
    assert!(cell(&t, 7, 0).is_zero());
}

#[test]
fn editing_save_cursor_protected_pen() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.set_protected_mode(ProtectedMode::Iso);
    t.save_cursor();
    t.set_protected_mode(ProtectedMode::Off);
    t.restore_cursor();
    assert!(t.screens.active().cursor.protected);
}

#[test]
fn erase_chars_keeps_protected_wide_text() {
    for text in ["中", "中\u{301}"] {
        for (prefix, row_index, column) in [("", 1, 1), ("", 1, 2), ("...", 2, 1), ("...", 2, 2)] {
            let mut t = Terminal::new(4, 3, 0).unwrap();
            t.print_string(prefix).unwrap();
            t.set_protected_mode(ProtectedMode::Iso);
            t.screens
                .active_mut()
                .start_hyperlink(b"wide", None)
                .unwrap();
            t.print_string(text).unwrap();
            t.set_protected_mode(ProtectedMode::Off);
            let before_head = cell(&t, 0, row_index - 1);
            let before_tail = cell(&t, 1, row_index - 1);
            t.set_cursor_pos(row_index as usize, column);
            t.erase_chars(1);
            assert_eq!(cell(&t, 0, row_index - 1), before_head);
            assert_eq!(cell(&t, 1, row_index - 1), before_tail);
            if prefix.is_empty() {
                assert_text(&t, text);
            } else {
                assert_text(&t, &format!("...\n{text}"));
            }
            assert!(cell(&t, 0, row_index - 1).hyperlink());
        }
    }
}

#[test]
fn erase_chars_clears_unprotected_wide_boundaries() {
    for mode in [ProtectedMode::Off, ProtectedMode::Dec] {
        for column in [1, 2] {
            let mut t = Terminal::new(4, 2, 0).unwrap();
            t.set_protected_mode(mode);
            t.print_string("中").unwrap();
            t.set_cursor_pos(1, column);
            t.erase_chars(1);
            assert_text(&t, "");
            assert!(cell(&t, 0, 0).is_zero());
            assert!(cell(&t, 1, 0).is_zero());
            assert_eq!(
                t.screens.active().cursor.x,
                u16::try_from(column - 1).unwrap()
            );
            assert!(!t.screens.active().cursor.pending_wrap);
        }
    }
}
