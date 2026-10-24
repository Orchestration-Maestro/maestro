//! Display clearing, selective protection and retained prompt history.

use super::support::terminal::{assert_text, cell, clear_dirty, dirty};
use maestro_terminal::{Coordinate, Point, Terminal, csi::EraseDisplay, page::row::SemanticPrompt};

#[test]
fn prompt_clear_skips_trailing_empty_rows() {
    for y in [0, 2] {
        let mut t = Terminal::new(5, 3, 10000).unwrap();
        t.set_cursor_pos(y + 1, 1);
        t.print('$').unwrap();
        let screen = t.screens.active_mut();
        let pin = screen
            .pages
            .pin(Point::Active(Coordinate {
                x: 0,
                y: u32::try_from(y).unwrap(),
            }))
            .unwrap();
        pin.row_and_cell_mut(&mut screen.pages)
            .unwrap()
            .0
            .set_semantic_prompt(SemanticPrompt::Prompt);
        t.set_cursor_pos(1, 1);
        t.erase_display(EraseDisplay::Complete, false);
        assert!(t.screens.active().pages.history_rows() > 0);
        assert_text(&t, "");
    }
}

#[test]
fn editing_erase_display_simple_erase_below() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.print_string("ABC").unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print_string("DEF").unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print_string("GHI").unwrap();
    t.set_cursor_pos(2, 2);
    clear_dirty(&mut t);
    t.erase_display(EraseDisplay::Below, false);
    assert!(!dirty(&t, 0));
    assert!(dirty(&t, 1));
    assert!(dirty(&t, 2));
    assert_text(&t, "ABC\nD");
}

#[test]
fn editing_erase_display_erase_below_preserves_sgr_bg() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.print_string("ABC").unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print_string("DEF").unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print_string("GHI").unwrap();
    t.set_cursor_pos(2, 2);
    let rgb = maestro_terminal::color::Rgb { r: 255, g: 0, b: 0 };
    t.screens
        .active_mut()
        .set_attribute(maestro_terminal::sgr::Attribute::DirectColorBg(rgb))
        .unwrap();
    t.erase_display(EraseDisplay::Below, false);
    assert_text(&t, "ABC\nD");
    for x in 1..5 {
        assert_eq!(
            cell(&t, x, 1).content(),
            maestro_terminal::page::cell::Content::BgColorRgb(rgb)
        );
    }
}

#[test]
fn editing_erase_display_below_protected_attributes_respected_with_iso() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.set_protected_mode(maestro_terminal::ansi::ProtectedMode::Iso);
    t.print_string("ABC").unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print_string("DEF").unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print_string("GHI").unwrap();
    t.set_cursor_pos(2, 2);
    t.erase_display(EraseDisplay::Below, false);
    assert_text(&t, "ABC\nDEF\nGHI");
}

#[test]
fn editing_erase_display_below_protected_attributes_ignored_with_dec_most_recent() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.set_protected_mode(maestro_terminal::ansi::ProtectedMode::Iso);
    t.print_string("ABC").unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print_string("DEF").unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print_string("GHI").unwrap();
    t.set_cursor_pos(2, 2);
    t.set_protected_mode(maestro_terminal::ansi::ProtectedMode::Dec);
    t.set_protected_mode(maestro_terminal::ansi::ProtectedMode::Off);
    t.erase_display(EraseDisplay::Below, false);
    assert_text(&t, "ABC\nD");
}

#[test]
fn editing_erase_display_below_protected_attributes_ignored_with_dec_set() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.set_protected_mode(maestro_terminal::ansi::ProtectedMode::Dec);
    t.print_string("ABC").unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print_string("DEF").unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print_string("GHI").unwrap();
    t.set_cursor_pos(2, 2);
    t.erase_display(EraseDisplay::Below, false);
    assert_text(&t, "ABC\nD");
}

#[test]
fn editing_erase_display_below_protected_attributes_respected_with_force() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.set_protected_mode(maestro_terminal::ansi::ProtectedMode::Dec);
    t.print_string("ABC").unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print_string("DEF").unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print_string("GHI").unwrap();
    t.set_cursor_pos(2, 2);
    t.erase_display(EraseDisplay::Below, true);
    assert_text(&t, "ABC\nDEF\nGHI");
}

#[test]
fn editing_erase_display_simple_erase_above() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.print_string("ABC").unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print_string("DEF").unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print_string("GHI").unwrap();
    t.set_cursor_pos(2, 2);
    clear_dirty(&mut t);
    t.erase_display(EraseDisplay::Above, false);
    assert!(dirty(&t, 0));
    assert!(dirty(&t, 1));
    assert!(!dirty(&t, 2));
    assert_text(&t, "\n  F\nGHI");
}

#[test]
fn editing_erase_display_erase_above_preserves_sgr_bg() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.print_string("ABC").unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print_string("DEF").unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print_string("GHI").unwrap();
    t.set_cursor_pos(2, 2);
    let rgb = maestro_terminal::color::Rgb { r: 255, g: 0, b: 0 };
    t.screens
        .active_mut()
        .set_attribute(maestro_terminal::sgr::Attribute::DirectColorBg(rgb))
        .unwrap();
    t.erase_display(EraseDisplay::Above, false);
    assert_text(&t, "\n  F\nGHI");
    for x in 0..2 {
        assert_eq!(
            cell(&t, x, 1).content(),
            maestro_terminal::page::cell::Content::BgColorRgb(rgb)
        );
    }
}

#[test]
fn editing_erase_display_above_protected_attributes_respected_with_iso() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.set_protected_mode(maestro_terminal::ansi::ProtectedMode::Iso);
    t.print_string("ABC").unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print_string("DEF").unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print_string("GHI").unwrap();
    t.set_cursor_pos(2, 2);
    t.erase_display(EraseDisplay::Above, false);
    assert_text(&t, "ABC\nDEF\nGHI");
}

#[test]
fn editing_erase_display_above_protected_attributes_ignored_with_dec_most_recent() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.set_protected_mode(maestro_terminal::ansi::ProtectedMode::Iso);
    t.print_string("ABC").unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print_string("DEF").unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print_string("GHI").unwrap();
    t.set_cursor_pos(2, 2);
    t.set_protected_mode(maestro_terminal::ansi::ProtectedMode::Dec);
    t.set_protected_mode(maestro_terminal::ansi::ProtectedMode::Off);
    t.erase_display(EraseDisplay::Above, false);
    assert_text(&t, "\n  F\nGHI");
}

#[test]
fn editing_erase_display_above_protected_attributes_ignored_with_dec_set() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.set_protected_mode(maestro_terminal::ansi::ProtectedMode::Dec);
    t.print_string("ABC").unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print_string("DEF").unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print_string("GHI").unwrap();
    t.set_cursor_pos(2, 2);
    t.erase_display(EraseDisplay::Above, false);
    assert_text(&t, "\n  F\nGHI");
}

#[test]
fn editing_erase_display_above_protected_attributes_respected_with_force() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.set_protected_mode(maestro_terminal::ansi::ProtectedMode::Dec);
    t.print_string("ABC").unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print_string("DEF").unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print_string("GHI").unwrap();
    t.set_cursor_pos(2, 2);
    t.erase_display(EraseDisplay::Above, true);
    assert_text(&t, "ABC\nDEF\nGHI");
}

#[test]
fn editing_erase_display_below_split_multi_cell() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.print_string("AB橋C").unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print_string("DE橋F").unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print_string("GH橋I").unwrap();
    t.set_cursor_pos(2, 4);
    t.erase_display(EraseDisplay::Below, false);
    assert_text(&t, "AB橋C\nDE");
}

#[test]
fn editing_erase_display_protected_complete() {
    let mut t = Terminal::new(10, 5, 0).unwrap();
    t.print('A').unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print_string("123456789").unwrap();
    t.set_cursor_pos(2, 6);
    t.set_protected_mode(maestro_terminal::ansi::ProtectedMode::Dec);
    t.print('X').unwrap();
    t.set_cursor_pos(2, 4);
    clear_dirty(&mut t);
    t.erase_display(EraseDisplay::Complete, true);
    assert_text(&t, "\n     X");
    for y in 0..5 {
        assert!(dirty(&t, y));
    }
}

#[test]
fn editing_erase_display_protected_below() {
    let mut t = Terminal::new(10, 5, 0).unwrap();
    t.print('A').unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print_string("123456789").unwrap();
    t.set_cursor_pos(2, 6);
    t.set_protected_mode(maestro_terminal::ansi::ProtectedMode::Dec);
    t.print('X').unwrap();
    t.set_cursor_pos(2, 4);
    clear_dirty(&mut t);
    t.erase_display(EraseDisplay::Below, true);
    assert_text(&t, "A\n123  X");
}

#[test]
fn editing_erase_display_protected_above() {
    let mut t = Terminal::new(10, 3, 0).unwrap();
    t.print('A').unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print_string("123456789").unwrap();
    t.set_cursor_pos(2, 6);
    t.set_protected_mode(maestro_terminal::ansi::ProtectedMode::Dec);
    t.print('X').unwrap();
    t.set_cursor_pos(2, 8);
    clear_dirty(&mut t);
    t.erase_display(EraseDisplay::Above, true);
    assert_text(&t, "\n     X  9");
}

#[test]
fn editing_erase_display_scroll_complete() {
    let mut t = Terminal::new(10, 5, 10000).unwrap();
    t.print('A').unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.erase_display(EraseDisplay::ScrollComplete, false);
    assert_text(&t, "");
}

#[test]
fn editing_erase_display_complete_preserves_cursor() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.screens
        .active_mut()
        .set_attribute(maestro_terminal::sgr::Attribute::Bold)
        .unwrap();
    t.print_string("AAAA").unwrap();
    assert_ne!(t.screens.active().cursor.style_id, 0);
    t.erase_display(EraseDisplay::Complete, false);
    assert_ne!(t.screens.active().cursor.style_id, 0);
}

#[test]
fn editing_erase_display_above_split_multi_cell() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    for text in ["AB橋C", "DE橋F", "GH橋I"] {
        if t.screens.active().cursor.pending_wrap {
            t.carriage_return();
            t.linefeed().unwrap();
        }
        t.print_string(text).unwrap();
    }
    t.set_cursor_pos(2, 3);
    t.erase_display(EraseDisplay::Above, false);
    assert_text(&t, "\n    F\nGH橋I");
}

/// Mark a stored active row as a shell prompt without changing cursor position.
fn mark_prompt(t: &mut Terminal, y: u32) {
    let screen = t.screens.active_mut();
    let pin = screen
        .pages
        .pin(Point::Active(Coordinate { x: 0, y }))
        .unwrap();
    pin.row_and_cell_mut(&mut screen.pages)
        .unwrap()
        .0
        .set_semantic_prompt(SemanticPrompt::Prompt);
}

#[test]
fn prompt_clear_keeps_text_in_history() {
    for home in [false, true] {
        let mut t = Terminal::new(5, 3, 10000).unwrap();
        t.print('$').unwrap();
        mark_prompt(&mut t, 0);
        if home {
            t.set_cursor_pos(1, 1);
        }
        t.erase_display(EraseDisplay::Complete, false);
        assert_text(&t, "");
        let screen = t.screens.active();
        let pin = screen
            .pages
            .pin(Point::History(Coordinate::default()))
            .unwrap();
        assert_eq!(pin.row_and_cell(&screen.pages).unwrap().1.codepoint(), '$');
    }
}

#[test]
fn later_output_prevents_prompt_scroll_clear() {
    let mut t = Terminal::new(5, 3, 10000).unwrap();
    t.print('$').unwrap();
    mark_prompt(&mut t, 0);
    t.set_cursor_pos(2, 1);
    t.print_string("output").unwrap();
    t.set_cursor_pos(1, 1);
    t.erase_display(EraseDisplay::Complete, false);
    assert_text(&t, "");
    assert_eq!(t.screens.active().pages.history_rows(), 0);
}

#[test]
fn alternate_prompt_clear_does_not_create_history() {
    use maestro_terminal::{screen::Options, screens::Key};
    let mut t = Terminal::new(5, 3, 10000).unwrap();
    t.erase_display(EraseDisplay::Complete, false);
    assert_eq!(t.screens.active().pages.history_rows(), 0);
    t.screens.get_init(
        Key::Alternate,
        Options {
            cols: std::num::NonZeroU16::new(5).unwrap(),
            rows: std::num::NonZeroU16::new(3).unwrap(),
            max_scrollback: 0,
        },
    );
    t.screens.switch_to(Key::Alternate).unwrap();
    t.print('$').unwrap();
    mark_prompt(&mut t, 0);
    t.erase_display(EraseDisplay::Complete, false);
    assert_text(&t, "");
    assert_eq!(t.screens.active().pages.history_rows(), 0);
}

#[test]
fn prompt_row_selection_uses_scroll_clear_emptiness() {
    for kind in 0..6 {
        let mut t = Terminal::new(5, 3, 10000).unwrap();
        t.print('$').unwrap();
        mark_prompt(&mut t, 0);
        t.set_cursor_pos(2, 1);
        write_candidate_row(&mut t, kind);
        t.erase_display(EraseDisplay::Complete, false);
        assert_eq!(
            t.screens.active().pages.history_rows() > 0,
            kind < 2,
            "kind {kind}"
        );
        assert_text(&t, "");
    }
}

/// Populate a candidate row with the cell classes used by prompt history selection.
fn write_candidate_row(t: &mut Terminal, kind: u8) {
    use maestro_terminal::{
        page::{
            CellPosition,
            cell::{Content, Wide},
        },
        sgr::Attribute,
    };
    match kind {
        1 => {
            t.screens
                .active_mut()
                .set_attribute(Attribute::Bold)
                .unwrap();
            t.print('X').unwrap();
        }
        2 => t.print(' ').unwrap(),
        3 => {
            t.screens
                .active_mut()
                .set_attribute(Attribute::Palette256Bg(1))
                .unwrap();
            t.erase_chars(1);
            t.screens
                .active_mut()
                .set_attribute(Attribute::Unset)
                .unwrap();
        }
        4 => t.print_string("a\u{301}").unwrap(),
        _ => {}
    }
    let screen = t.screens.active_mut();
    let pin = screen
        .pages
        .pin(Point::Active(Coordinate { x: 0, y: 1 }))
        .unwrap();
    match kind {
        1 => pin
            .row_and_cell_mut(&mut screen.pages)
            .unwrap()
            .1
            .set_content(Content::Codepoint('\0')),
        4 => assert!(
            screen
                .pages
                .page(pin.node)
                .unwrap()
                .lookup_grapheme(CellPosition { x: 0, y: pin.y })
                .is_some()
        ),
        5 => pin
            .row_and_cell_mut(&mut screen.pages)
            .unwrap()
            .1
            .set_wide(Wide::SpacerHead),
        _ => {}
    }
}
