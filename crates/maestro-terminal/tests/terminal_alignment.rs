//! Screen alignment fills ignore protection and preserve only foreground/background.

use super::support::terminal::{assert_text, cell, dirty};
use maestro_terminal::{Mode, Terminal, ansi::ProtectedMode};

#[test]
fn editing_decaln() {
    let mut t = Terminal::new(2, 2, 0).unwrap();
    t.print_string("A\nB").unwrap();
    t.decaln().unwrap();
    assert_eq!(
        (t.screens.active().cursor.x, t.screens.active().cursor.y),
        (0, 0)
    );
    for y in 0..2 {
        assert!(dirty(&t, y));
    }
    assert_text(&t, "EE\nEE");
}

#[test]
fn editing_decaln_reset_margins() {
    let mut t = Terminal::new(3, 3, 0).unwrap();
    t.modes.set(Mode::Origin, true);
    t.set_top_and_bottom_margin(2, 3);
    t.decaln().unwrap();
    t.scroll_down(1).unwrap();
    assert_text(&t, "\nEEE\nEEE");
    assert!(!t.modes.get(Mode::Origin));
    assert_eq!((t.scrolling_region.left, t.scrolling_region.right), (0, 2));
}

#[test]
fn editing_decaln_preserves_color() {
    use maestro_terminal::{color::Rgb, page::cell::Content, sgr::Attribute, style::Color};
    let mut t = Terminal::new(3, 3, 0).unwrap();
    let rgb = Rgb { r: 255, g: 0, b: 0 };
    t.set_attribute(Attribute::DirectColorBg(rgb)).unwrap();
    t.set_attribute(Attribute::Palette256Fg(3)).unwrap();
    t.set_attribute(Attribute::Bold).unwrap();
    t.modes.set(Mode::Origin, true);
    t.set_top_and_bottom_margin(2, 3);
    t.decaln().unwrap();
    assert!(!t.screens.active().cursor.style.flags.bold());
    assert_eq!(t.screens.active().cursor.style.fg_color, Color::Palette(3));
    assert_eq!(t.screens.active().cursor.style.bg_color, Color::Rgb(rgb));
    t.scroll_down(1).unwrap();
    assert_text(&t, "\nEEE\nEEE");
    assert_eq!(cell(&t, 0, 0).content(), Content::BgColorRgb(rgb));
}

#[test]
fn editing_decaln_resets_graphemes_with_protected_mode() {
    let mut t = Terminal::new(3, 3, 0).unwrap();
    t.set_protected_mode(ProtectedMode::Iso);
    t.modes.set(Mode::GraphemeCluster, true);
    t.print_string("👨\u{200d}👩\u{200d}👧").unwrap();
    t.decaln().unwrap();
    assert_eq!(
        (t.screens.active().cursor.x, t.screens.active().cursor.y),
        (0, 0)
    );
    assert!(t.screens.active().cursor.protected);
    assert_eq!(t.screens.active().protected_mode, ProtectedMode::Iso);
    for y in 0..3 {
        assert!(dirty(&t, y));
        for x in 0..3 {
            assert!(!cell(&t, x, y).protected());
            assert!(!cell(&t, x, y).has_grapheme());
        }
    }
    assert_text(&t, "EEE\nEEE\nEEE");
}
