//! Alternate-screen mode ordering and independent retained state.

use super::support::terminal::assert_text;
use maestro_terminal::{Terminal, screen_set::Key, terminal::SwitchScreenMode};

fn plain_mode(mode: SwitchScreenMode) {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.print_string("1A").unwrap();
    t.switch_screen_mode(mode, true).unwrap();
    assert_eq!(t.screens.active_key(), Key::Alternate);
    assert_text(&t, "");
    t.print_string("2B").unwrap();
    assert_text(&t, "  2B");
    t.switch_screen_mode(mode, false).unwrap();
    assert_eq!(t.screens.active_key(), Key::Primary);
    assert_text(&t, "1A");
    if mode == SwitchScreenMode::Mode1049 {
        t.print_string("C").unwrap();
        assert_text(&t, "1AC");
    }
    t.switch_screen_mode(mode, true).unwrap();
    assert_eq!(t.screens.active_key(), Key::Alternate);
    assert_text(
        &t,
        if mode == SwitchScreenMode::Mode47 {
            "  2B"
        } else {
            ""
        },
    );
}

fn cursor_colors(mode: SwitchScreenMode) {
    use maestro_terminal::{color::Rgb, sgr::Attribute};
    let mut t = Terminal::new(5, 5, 0).unwrap();
    for (enabled, rgb) in [
        (
            true,
            Rgb {
                r: 255,
                g: 0,
                b: 127,
            },
        ),
        (false, Rgb { r: 0, g: 255, b: 0 }),
    ] {
        t.screens
            .active_mut()
            .set_attribute(Attribute::DirectColorFg(rgb))
            .unwrap();
        let style = t.screens.active().cursor.style;
        t.switch_screen_mode(mode, enabled).unwrap();
        assert_eq!(
            t.screens.active_key(),
            if enabled {
                Key::Alternate
            } else {
                Key::Primary
            }
        );
        let s = t.screens.active();
        assert_eq!(s.cursor.style, style);
        assert_ne!(s.cursor.style_id, 0);
    }
}

#[test]
fn editing_mode_47_alt_screen_plain() {
    plain_mode(SwitchScreenMode::Mode47);
}
#[test]
fn editing_mode_1047_alt_screen_plain() {
    plain_mode(SwitchScreenMode::Mode1047);
}
#[test]
fn editing_mode_1049_alt_screen_plain() {
    plain_mode(SwitchScreenMode::Mode1049);
}
#[test]
fn editing_mode_47_copies_cursor_both_directions() {
    cursor_colors(SwitchScreenMode::Mode47);
}
#[test]
fn editing_mode_1047_copies_cursor_both_directions() {
    cursor_colors(SwitchScreenMode::Mode1047);
}

#[test]
fn repeated_screen_modes_keep_save_and_clear_order() {
    for mode in [
        SwitchScreenMode::Mode47,
        SwitchScreenMode::Mode1047,
        SwitchScreenMode::Mode1049,
    ] {
        let mut t = Terminal::new(8, 3, 0).unwrap();
        t.print_string("P").unwrap();
        t.switch_screen_mode(mode, true).unwrap();
        t.print_string("A").unwrap();
        t.switch_screen_mode(mode, true).unwrap();
        assert_text(
            &t,
            if mode == SwitchScreenMode::Mode1049 {
                ""
            } else {
                " A"
            },
        );
        if mode == SwitchScreenMode::Mode1049 {
            assert_eq!(t.screens.active().saved_cursor.unwrap().x, 2);
            assert_eq!(
                t.screens.get(Key::Primary).unwrap().saved_cursor.unwrap().x,
                1
            );
        } else {
            assert!(t.screens.active().saved_cursor.is_none());
        }
        t.switch_screen_mode(mode, false).unwrap();
        assert_text(&t, "P");
        let x = t.screens.active().cursor.x;
        t.set_cursor_pos(2, 4);
        t.switch_screen_mode(mode, false).unwrap();
        assert_eq!(
            t.screens.active().cursor.x,
            if mode == SwitchScreenMode::Mode1049 {
                x
            } else {
                3
            }
        );
        t.switch_screen_mode(mode, true).unwrap();
        assert_text(
            &t,
            if mode == SwitchScreenMode::Mode47 {
                " A"
            } else {
                ""
            },
        );
    }
}

#[test]
fn screen_switch_ends_old_link_and_carries_charsets() {
    use maestro_terminal::{Charset, CharsetSlot as Slots};
    let mut t = Terminal::new(5, 3, 100_000).unwrap();
    t.screens
        .active_mut()
        .charset
        .charsets
        .set(Slots::G0, Charset::British);
    let charset = t.screens.active().charset;
    t.screens
        .active_mut()
        .start_hyperlink(b"https://example.test", None)
        .unwrap();
    t.flags.dirty.clear = false;
    assert_eq!(t.switch_screen(Key::Primary), None);
    assert!(t.screens.active().cursor.hyperlink.is_some());
    assert!(!t.flags.dirty.clear);
    assert_eq!(t.switch_screen(Key::Alternate), Some(Key::Primary));
    assert!(
        t.screens
            .get(Key::Primary)
            .unwrap()
            .cursor
            .hyperlink
            .is_none()
    );
    assert!(t.screens.active().cursor.hyperlink.is_none());
    assert_eq!(t.screens.active().charset, charset);
    assert!(t.screens.active().no_scrollback);
    assert!(t.flags.dirty.clear);
    assert_eq!(t.switch_screen(Key::Primary), Some(Key::Alternate));
    assert_eq!(t.screens.active().charset, charset);
}
