//! Line erasure modes, selective protection and pending wrap.

use super::support::terminal::{assert_text, cell, clear_dirty, dirty, row};
use maestro_terminal::{Terminal, ansi::ProtectedMode, csi::EraseLine};

#[test]
fn conditional_line_erase_clears_nonwrapped_cells() {
    let mut t = Terminal::new(5, 2, 0).unwrap();
    t.print_string("ABCDE").unwrap();
    t.set_cursor_pos(1, 3);
    clear_dirty(&mut t);
    t.erase_line(EraseLine::RightUnlessPendingWrap, false);
    for x in 2..5 {
        assert!(cell(&t, x, 0).is_zero());
    }
    assert!(dirty(&t, 0));
    assert!(!dirty(&t, 1));
}

#[test]
fn editing_erase_line_simple_erase_right() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.print_string("ABCDE").unwrap();
    t.set_cursor_pos(1, 3);
    clear_dirty(&mut t);
    t.erase_line(EraseLine::Right, false);
    assert!(dirty(&t, 0));
    assert_text(&t, "AB");
}

#[test]
fn editing_erase_line_simple_erase_left() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.print_string("ABCDE").unwrap();
    t.set_cursor_pos(1, 3);
    clear_dirty(&mut t);
    t.erase_line(EraseLine::Left, false);
    assert!(dirty(&t, 0));
    assert_text(&t, "   DE");
}

#[test]
fn editing_erase_line_resets_pending_wrap() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.print_string("ABCDE").unwrap();
    assert!(t.screens.active().cursor.pending_wrap);
    t.erase_line(EraseLine::Right, false);
    assert!(!t.screens.active().cursor.pending_wrap);
    t.print('B').unwrap();
    assert_text(&t, "ABCDB");
}

#[test]
fn editing_erase_line_resets_wrap() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.print_string("ABCDE123").unwrap();
    assert!(row(&t, 0).wrap());
    t.set_cursor_pos(1, 1);
    t.erase_line(EraseLine::Right, false);
    assert!(!row(&t, 0).wrap());
    t.print('X').unwrap();
    assert_text(&t, "X\n123");
}

#[test]
fn editing_erase_line_left_resets_wrap() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.print_string("ABCDE").unwrap();
    assert!(t.screens.active().cursor.pending_wrap);
    clear_dirty(&mut t);
    t.erase_line(EraseLine::Left, false);
    assert!(dirty(&t, 0));
    assert!(!t.screens.active().cursor.pending_wrap);
    t.print('B').unwrap();
    assert_text(&t, "    B");
}

#[test]
fn editing_erase_line_right_preserves_background_sgr() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.print_string("ABCDE").unwrap();
    t.set_cursor_pos(1, 2);
    let rgb = maestro_terminal::color::Rgb { r: 255, g: 0, b: 0 };
    t.screens
        .active_mut()
        .set_attribute(maestro_terminal::sgr::Attribute::DirectColorBg(rgb))
        .unwrap();
    t.erase_line(EraseLine::Right, false);
    assert_text(&t, "A");
    for x in 1..5 {
        assert_eq!(
            cell(&t, x, 0).content(),
            maestro_terminal::page::cell::Content::BgColorRgb(rgb)
        );
    }
}

#[test]
fn editing_erase_line_left_preserves_background_sgr() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.print_string("ABCDE").unwrap();
    t.set_cursor_pos(1, 2);
    let rgb = maestro_terminal::color::Rgb { r: 255, g: 0, b: 0 };
    t.screens
        .active_mut()
        .set_attribute(maestro_terminal::sgr::Attribute::DirectColorBg(rgb))
        .unwrap();
    t.erase_line(EraseLine::Left, false);
    assert_text(&t, "  CDE");
    for x in 0..2 {
        assert_eq!(
            cell(&t, x, 0).content(),
            maestro_terminal::page::cell::Content::BgColorRgb(rgb)
        );
    }
}

#[test]
fn editing_erase_line_complete_preserves_background_sgr() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.print_string("ABCDE").unwrap();
    t.set_cursor_pos(1, 2);
    let rgb = maestro_terminal::color::Rgb { r: 255, g: 0, b: 0 };
    t.screens
        .active_mut()
        .set_attribute(maestro_terminal::sgr::Attribute::DirectColorBg(rgb))
        .unwrap();
    t.erase_line(EraseLine::Complete, false);
    assert_text(&t, "");
    for x in 0..5 {
        assert_eq!(
            cell(&t, x, 0).content(),
            maestro_terminal::page::cell::Content::BgColorRgb(rgb)
        );
    }
}

#[test]
fn editing_erase_line_right_wide_character() {
    let mut t = Terminal::new(10, 5, 0).unwrap();
    t.print_string("AB橋DE").unwrap();
    t.set_cursor_pos(1, 4);
    clear_dirty(&mut t);
    t.erase_line(EraseLine::Right, false);
    assert!(dirty(&t, 0));
    assert_text(&t, "AB");
}

#[test]
fn editing_erase_line_left_wide_character() {
    let mut t = Terminal::new(10, 5, 0).unwrap();
    t.print_string("AB橋DE").unwrap();
    t.set_cursor_pos(1, 3);
    clear_dirty(&mut t);
    t.erase_line(EraseLine::Left, false);
    assert!(dirty(&t, 0));
    assert_text(&t, "    DE");
}

#[test]
fn editing_erase_line_right_protected_attributes_respected_with_iso() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.set_protected_mode(ProtectedMode::Iso);
    t.print_string("ABC").unwrap();
    t.set_cursor_pos(1, 1);
    clear_dirty(&mut t);
    t.erase_line(EraseLine::Right, false);
    assert!(dirty(&t, 0));
    assert_text(&t, "ABC");
}

#[test]
fn editing_erase_line_right_protected_attributes_ignored_with_dec_most_recent() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.set_protected_mode(ProtectedMode::Iso);
    t.print_string("ABC").unwrap();
    t.set_protected_mode(ProtectedMode::Dec);
    t.set_protected_mode(ProtectedMode::Off);
    t.set_cursor_pos(1, 2);
    clear_dirty(&mut t);
    t.erase_line(EraseLine::Right, false);
    assert!(dirty(&t, 0));
    assert_text(&t, "A");
}

#[test]
fn editing_erase_line_right_protected_attributes_ignored_with_dec_set() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.set_protected_mode(ProtectedMode::Dec);
    t.print_string("ABC").unwrap();
    t.set_cursor_pos(1, 2);
    clear_dirty(&mut t);
    t.erase_line(EraseLine::Right, false);
    assert!(dirty(&t, 0));
    assert_text(&t, "A");
}

#[test]
fn editing_erase_line_left_protected_attributes_respected_with_iso() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.set_protected_mode(ProtectedMode::Iso);
    t.print_string("ABC").unwrap();
    t.set_cursor_pos(1, 1);
    clear_dirty(&mut t);
    t.erase_line(EraseLine::Left, false);
    assert!(dirty(&t, 0));
    assert_text(&t, "ABC");
}

#[test]
fn editing_erase_line_left_protected_attributes_ignored_with_dec_most_recent() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.set_protected_mode(ProtectedMode::Iso);
    t.print_string("ABC").unwrap();
    t.set_protected_mode(ProtectedMode::Dec);
    t.set_protected_mode(ProtectedMode::Off);
    t.set_cursor_pos(1, 2);
    clear_dirty(&mut t);
    t.erase_line(EraseLine::Left, false);
    assert!(dirty(&t, 0));
    assert_text(&t, "  C");
}

#[test]
fn editing_erase_line_left_protected_attributes_ignored_with_dec_set() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.set_protected_mode(ProtectedMode::Dec);
    t.print_string("ABC").unwrap();
    t.set_cursor_pos(1, 2);
    clear_dirty(&mut t);
    t.erase_line(EraseLine::Left, false);
    assert!(dirty(&t, 0));
    assert_text(&t, "  C");
}

#[test]
fn editing_erase_line_complete_protected_attributes_respected_with_iso() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.set_protected_mode(ProtectedMode::Iso);
    t.print_string("ABC").unwrap();
    t.set_cursor_pos(1, 1);
    clear_dirty(&mut t);
    t.erase_line(EraseLine::Complete, false);
    assert!(dirty(&t, 0));
    assert_text(&t, "ABC");
}

#[test]
fn editing_erase_line_complete_protected_attributes_ignored_with_dec_most_recent() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.set_protected_mode(ProtectedMode::Iso);
    t.print_string("ABC").unwrap();
    t.set_protected_mode(ProtectedMode::Dec);
    t.set_protected_mode(ProtectedMode::Off);
    t.set_cursor_pos(1, 2);
    clear_dirty(&mut t);
    t.erase_line(EraseLine::Complete, false);
    assert!(dirty(&t, 0));
    assert_text(&t, "");
}

#[test]
fn editing_erase_line_complete_protected_attributes_ignored_with_dec_set() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.set_protected_mode(ProtectedMode::Dec);
    t.print_string("ABC").unwrap();
    t.set_cursor_pos(1, 2);
    clear_dirty(&mut t);
    t.erase_line(EraseLine::Complete, false);
    assert!(dirty(&t, 0));
    assert_text(&t, "");
}

#[test]
fn editing_erase_line_right_protected_requested() {
    let mut t = Terminal::new(10, 5, 0).unwrap();
    t.print_string("12345678").unwrap();
    t.set_cursor_pos(1, 6);
    t.set_protected_mode(ProtectedMode::Dec);
    t.print('X').unwrap();
    t.set_cursor_pos(1, 4);
    clear_dirty(&mut t);
    t.erase_line(EraseLine::Right, true);
    assert!(dirty(&t, 0));
    assert_text(&t, "123  X");
}

#[test]
fn editing_erase_line_left_protected_requested() {
    let mut t = Terminal::new(10, 5, 0).unwrap();
    t.print_string("123456789").unwrap();
    t.set_cursor_pos(1, 6);
    t.set_protected_mode(ProtectedMode::Dec);
    t.print('X').unwrap();
    t.set_cursor_pos(1, 8);
    clear_dirty(&mut t);
    t.erase_line(EraseLine::Left, true);
    assert!(dirty(&t, 0));
    assert_text(&t, "     X  9");
}

#[test]
fn editing_erase_line_complete_protected_requested() {
    let mut t = Terminal::new(10, 5, 0).unwrap();
    t.print_string("123456789").unwrap();
    t.set_cursor_pos(1, 6);
    t.set_protected_mode(ProtectedMode::Dec);
    t.print('X').unwrap();
    t.set_cursor_pos(1, 8);
    clear_dirty(&mut t);
    t.erase_line(EraseLine::Complete, true);
    assert!(dirty(&t, 0));
    assert_text(&t, "     X");
}

#[test]
fn conditional_line_erase_changes_visible_text() {
    for mode in [EraseLine::Right, EraseLine::RightUnlessPendingWrap] {
        let mut t = Terminal::new(5, 2, 0).unwrap();
        t.print_string("ABCDE").unwrap();
        t.set_cursor_pos(1, 3);
        t.erase_line(mode, false);
        assert_text(&t, "AB");
    }
}

#[test]
fn conditional_line_erase_keeps_pending_wrap() {
    for (mode, requested) in [(ProtectedMode::Iso, false), (ProtectedMode::Dec, true)] {
        let mut t = Terminal::new(5, 2, 0).unwrap();
        t.screens
            .active_mut()
            .start_hyperlink(b"retained", None)
            .unwrap();
        t.set_protected_mode(mode);
        t.print_string("ABCDE").unwrap();
        clear_dirty(&mut t);
        let cells: Vec<_> = (0..5).map(|x| cell(&t, x, 0)).collect();
        let before = row(&t, 0);
        t.erase_line(EraseLine::RightUnlessPendingWrap, requested);
        assert_eq!(
            (t.screens.active().cursor.x, t.screens.active().cursor.y),
            (4, 0)
        );
        assert!(t.screens.active().cursor.pending_wrap);
        assert_eq!(row(&t, 0), before);
        assert_eq!((0..5).map(|x| cell(&t, x, 0)).collect::<Vec<_>>(), cells);
        assert!(!dirty(&t, 0));
        assert!(!dirty(&t, 1));
        assert_text(&t, "ABCDE");
        t.set_cursor_pos(1, 3);
        t.erase_line(EraseLine::RightUnlessPendingWrap, requested);
        assert_text(&t, "ABCDE");
        assert!(dirty(&t, 0));
    }
}

#[test]
fn unknown_line_erase_is_a_noop() {
    let mut t = Terminal::new(5, 2, 0).unwrap();
    t.print_string("ABCDE").unwrap();
    clear_dirty(&mut t);
    let before = row(&t, 0);
    t.erase_line(EraseLine::Unknown(99), false);
    assert_text(&t, "ABCDE");
    assert_eq!(
        (t.screens.active().cursor.x, t.screens.active().cursor.y),
        (4, 0)
    );
    assert!(t.screens.active().cursor.pending_wrap);
    assert_eq!(row(&t, 0), before);
    assert!(!dirty(&t, 0));
    assert!(!dirty(&t, 1));
}
