//! Direct printing entry points and retained cell ownership.

use super::support::terminal::{assert_text, cell, clear_dirty, dirty, row};
use maestro_terminal::page::cell::Wide;
use maestro_terminal::sgr::Attribute;
use maestro_terminal::{Coordinate, Mode, Point, Terminal, ansi::StatusDisplay};

fn text(t: &Terminal, x: u16, y: u32) -> char {
    let s = t.screens.active();
    s.pages
        .pin(Point::Active(Coordinate { x, y }))
        .unwrap()
        .row_and_cell(&s.pages)
        .unwrap()
        .1
        .codepoint()
}

#[test]
fn terminal_status_line_printing_leaves_main_untouched() {
    let mut t = Terminal::new(5, 3, 0).unwrap();
    t.status_display = StatusDisplay::StatusLine;
    t.print('a').unwrap();
    t.print_slice(&['b', 'c']).unwrap();
    assert_eq!(
        (t.screens.active().cursor.x, t.screens.active().cursor.y),
        (0, 0)
    );
    assert_text(&t, "");
    assert!(!dirty(&t, 0));
    t.print_string("abc\nxyz").unwrap();
    assert_eq!(
        (t.screens.active().cursor.x, t.screens.active().cursor.y),
        (0, 1)
    );
    assert_eq!(text(&t, 0, 0), '\0');
    assert_eq!(text(&t, 0, 1), '\0');
    assert_eq!(t.previous_char, None);
}

#[test]
fn terminal_print_entry_points_share_pending_wrap_and_repeat() {
    let mut t = Terminal::new(3, 3, 0).unwrap();
    t.print_string("ab").unwrap();
    t.print_slice(&['c']).unwrap();
    assert!(t.screens.active().cursor.pending_wrap);
    t.print_repeat(0).unwrap();
    assert_eq!(text(&t, 2, 0), 'c');
    assert_eq!(text(&t, 0, 1), 'c');
    assert_eq!(
        (t.screens.active().cursor.x, t.screens.active().cursor.y),
        (1, 1)
    );
}

#[test]
fn terminal_print_cluster_width_and_suffix() {
    let mut t = Terminal::new(8, 3, 0).unwrap();
    t.print_string("a\u{301}❤\u{fe0f}界").unwrap();
    let s = t.screens.active();
    assert_eq!(s.cursor.x, 5);
    let pin = s
        .pages
        .pin(Point::Active(Coordinate { x: 1, y: 0 }))
        .unwrap();
    let page = s.pages.page(pin.node).unwrap();
    assert_eq!(
        page.lookup_grapheme(maestro_terminal::page::CellPosition { x: 1, y: pin.y }),
        Some(&['\u{fe0f}'][..])
    );
    assert_eq!(
        page.get_cells(pin.y)[1].wide(),
        maestro_terminal::page::cell::Wide::Wide
    );
    assert_eq!(
        page.get_cells(pin.y)[2].wide(),
        maestro_terminal::page::cell::Wide::SpacerTail
    );
}

#[test]
fn terminal_print_insert_mode_preserves_shifted_text() {
    let mut t = Terminal::new(6, 2, 0).unwrap();
    t.print_string("abcd").unwrap();
    t.set_cursor_pos(1, 2);
    t.modes.set(Mode::Insert, true);
    t.print('X').unwrap();
    assert_eq!(
        (
            text(&t, 0, 0),
            text(&t, 1, 0),
            text(&t, 2, 0),
            text(&t, 4, 0)
        ),
        ('a', 'X', 'b', 'd')
    );
}

#[test]
fn screen_input_with_no_control_characters() {
    let mut t = Terminal::new(40, 40, 0).unwrap();

    t.print_string("hello").unwrap();
    assert_eq!(t.screens.active().cursor.y, 0);
    assert_eq!(t.screens.active().cursor.x, 5);
    {
        assert_text(&t, "hello");
    }

    assert!(dirty(&t, 0));
    assert!(!dirty(&t, 1));
}

#[test]
fn screen_input_with_basic_wraparound() {
    let mut t = Terminal::new(5, 40, 0).unwrap();

    t.print_string("helloworldabc12").unwrap();
    assert_eq!(t.screens.active().cursor.y, 2);
    assert_eq!(t.screens.active().cursor.x, 4);
    assert!(t.screens.active().cursor.pending_wrap);
    {
        assert_text(&t, "hello\nworld\nabc12");
    }
}

#[test]
fn screen_input_with_basic_wraparound_dirty() {
    let mut t = Terminal::new(5, 40, 0).unwrap();

    t.print_string("hello").unwrap();
    assert!(dirty(&t, 0));
    clear_dirty(&mut t);
    t.print('w').unwrap();

    assert!(dirty(&t, 0));
    assert!(dirty(&t, 1));
}

#[test]
fn screen_input_that_forces_scroll() {
    let mut t = Terminal::new(1, 5, 0).unwrap();

    t.print_string("abcdef").unwrap();
    assert_eq!(t.screens.active().cursor.y, 4);
    assert_eq!(t.screens.active().cursor.x, 0);
    {
        assert_text(&t, "b\nc\nd\ne\nf");
    }
}

#[test]
fn screen_render_single_very_long_line() {
    let mut t = Terminal::new(5, 5, 0).unwrap();

    for _ in 0..1000 {
        t.print('x').unwrap();
    }
}

#[test]
fn screen_render_wide_char() {
    let mut t = Terminal::new(80, 80, 0).unwrap();

    t.print(char::from_u32(0x1F600).unwrap()).unwrap();
    assert_eq!(t.screens.active().cursor.y, 0);
    assert_eq!(t.screens.active().cursor.x, 2);

    {
        let value = cell(&t, 0, 0);

        assert_eq!(value.codepoint(), char::from_u32(0x1F600).unwrap());
        assert_eq!(value.wide(), Wide::Wide);
    }
    {
        let value = cell(&t, 1, 0);

        assert_eq!(value.wide(), Wide::SpacerTail);
    }

    assert!(dirty(&t, 0));
}

#[test]
fn screen_render_wide_char_at_edge_creates_spacer_head() {
    let mut t = Terminal::new(10, 10, 0).unwrap();

    t.set_cursor_pos(1, 10);
    t.print(char::from_u32(0x1F600).unwrap()).unwrap();
    assert_eq!(t.screens.active().cursor.y, 1);
    assert_eq!(t.screens.active().cursor.x, 2);

    {
        let value = cell(&t, 9, 0);

        assert_eq!(value.wide(), Wide::SpacerHead);
    }
    {
        let value = cell(&t, 0, 1);

        assert_eq!(value.codepoint(), char::from_u32(0x1F600).unwrap());
        assert_eq!(value.wide(), Wide::Wide);
    }
    {
        let value = cell(&t, 1, 1);

        assert_eq!(value.wide(), Wide::SpacerTail);
    }

    assert!(dirty(&t, 0));
    assert!(dirty(&t, 1));
}

#[test]
fn screen_render_wide_char_with_1_column_width() {
    let mut t = Terminal::new(1, 2, 0).unwrap();

    t.print('😀').unwrap();

    assert!(dirty(&t, 0));
}

#[test]
fn screen_render_wide_char_in_single_width_terminal() {
    let mut t = Terminal::new(1, 80, 0).unwrap();

    t.print(char::from_u32(0x1F600).unwrap()).unwrap();
    assert_eq!(t.screens.active().cursor.y, 0);
    assert_eq!(t.screens.active().cursor.x, 0);
    assert!(t.screens.active().cursor.pending_wrap);

    {
        let value = cell(&t, 0, 0);

        assert_eq!(value.codepoint(), char::from_u32(0).unwrap());
        assert_eq!(value.wide(), Wide::Narrow);
    }

    assert!(dirty(&t, 0));
}

#[test]
fn screen_render_over_wide_char_at_0_0() {
    let mut t = Terminal::new(80, 80, 0).unwrap();

    t.print(char::from_u32(0x1F600).unwrap()).unwrap();
    t.set_cursor_pos(0, 0);
    t.print('A').unwrap();

    assert_eq!(t.screens.active().cursor.y, 0);
    assert_eq!(t.screens.active().cursor.x, 1);

    {
        let value = cell(&t, 0, 0);

        assert_eq!(value.codepoint(), 'A');
        assert_eq!(value.wide(), Wide::Narrow);
    }
    {
        let value = cell(&t, 1, 0);

        assert_eq!(value.codepoint(), char::from_u32(0).unwrap());
        assert_eq!(value.wide(), Wide::Narrow);
    }

    assert!(dirty(&t, 0));
    assert!(!dirty(&t, 1));
}

#[test]
fn screen_render_over_wide_char_at_col_0_corrupts_previous_row() {
    let mut t = Terminal::new(10, 3, 0).unwrap();

    for _ in 0..10 {
        t.print(char::from_u32(0x4E2D).unwrap()).unwrap();
    }

    t.set_cursor_pos(2, 1);
    t.print('A').unwrap();

    {
        let value = cell(&t, 0, 1);
        assert_eq!(value.wide(), Wide::Narrow);
    }

    {
        let value = cell(&t, 8, 0);
        assert_eq!(value.wide(), Wide::Wide);
    }

    {
        let value = cell(&t, 9, 0);
        assert_eq!(value.wide(), Wide::SpacerTail);
    }
}

#[test]
fn screen_render_over_wide_spacer_tail() {
    let mut t = Terminal::new(5, 5, 0).unwrap();

    t.print('橋').unwrap();
    t.set_cursor_pos(1, 2);
    t.print('X').unwrap();

    {
        let value = cell(&t, 0, 0);

        assert_eq!(value.codepoint(), char::from_u32(0).unwrap());
        assert_eq!(value.wide(), Wide::Narrow);
    }
    {
        let value = cell(&t, 1, 0);

        assert_eq!(value.codepoint(), 'X');
        assert_eq!(value.wide(), Wide::Narrow);
    }

    {
        assert_text(&t, " X");
    }

    assert!(dirty(&t, 0));
}

#[test]
fn screen_default_style_is_empty() {
    let mut t = Terminal::new(5, 5, 0).unwrap();

    t.print('A').unwrap();

    {
        let value = cell(&t, 0, 0);

        assert_eq!(value.codepoint(), 'A');
        assert_eq!(value.style_id(), 0);
    }
}

#[test]
fn screen_render_with_style_marks_the_row_as_styled() {
    let mut t = Terminal::new(5, 5, 0).unwrap();

    t.set_attribute(Attribute::Bold).unwrap();
    t.print('A').unwrap();
    t.set_attribute(Attribute::Unset).unwrap();
    t.print('B').unwrap();

    {
        assert!(row(&t, 0).styled());
    }
}

#[test]
fn screen_insert_mode_with_space() {
    let mut t = Terminal::new(10, 2, 0).unwrap();

    t.print_string("hello").unwrap();
    t.set_cursor_pos(1, 2);
    t.modes.set(Mode::Insert, true);
    t.print('X').unwrap();

    {
        assert_text(&t, "hXello");
    }
}

#[test]
fn screen_insert_mode_does_not_wrap_pushed_characters() {
    let mut t = Terminal::new(5, 2, 0).unwrap();

    t.print_string("hello").unwrap();
    t.set_cursor_pos(1, 2);
    t.modes.set(Mode::Insert, true);
    t.print('X').unwrap();

    {
        assert_text(&t, "hXell");
    }
}

#[test]
fn screen_insert_mode_does_nothing_at_the_end_of_the_line() {
    let mut t = Terminal::new(5, 2, 0).unwrap();

    t.print_string("hello").unwrap();
    t.modes.set(Mode::Insert, true);
    t.print('X').unwrap();

    {
        assert_text(&t, "hello\nX");
    }
}

#[test]
fn screen_insert_mode_with_wide_characters() {
    let mut t = Terminal::new(5, 2, 0).unwrap();

    t.print_string("hello").unwrap();
    t.set_cursor_pos(1, 2);
    t.modes.set(Mode::Insert, true);
    t.print('😀').unwrap();

    {
        assert_text(&t, "h😀el");
    }
}

#[test]
fn screen_insert_mode_with_wide_characters_at_end() {
    let mut t = Terminal::new(5, 2, 0).unwrap();

    t.print_string("well").unwrap();
    t.modes.set(Mode::Insert, true);
    t.print('😀').unwrap();

    {
        assert_text(&t, "well\n😀");
    }
}

#[test]
fn screen_insert_mode_pushing_off_wide_character() {
    let mut t = Terminal::new(5, 2, 0).unwrap();

    t.print_string("123").unwrap();
    t.print('😀').unwrap();
    t.modes.set(Mode::Insert, true);
    t.set_cursor_pos(1, 1);
    t.print('X').unwrap();

    {
        assert_text(&t, "X123");
    }
}

#[test]
fn screen_setcursorpos_saturates_overflowing_origin_offsets() {
    let mut t = Terminal::new(10, 10, 0).unwrap();
    t.scrolling_region = maestro_terminal::terminal::ScrollingRegion {
        top: 2,
        bottom: 7,
        left: 3,
        right: 8,
    };
    t.modes.set(Mode::Origin, true);
    t.set_cursor_pos(usize::MAX, usize::MAX);
    assert_eq!(
        (t.screens.active().cursor.x, t.screens.active().cursor.y),
        (8, 7)
    );
}

#[test]
fn screen_input_unique_style_per_cell() {
    let mut t = Terminal::new(30, 30, 0).unwrap();
    for y in 0..30u8 {
        for x in 0..30u8 {
            t.set_cursor_pos(usize::from(y), usize::from(x));
            t.set_attribute(Attribute::DirectColorBg(maestro_terminal::color::Rgb {
                r: x,
                g: y,
                b: 0,
            }))
            .unwrap();
            t.print('x').unwrap();
        }
    }
}

#[test]
fn screen_zero_width_character_at_start() {
    let mut t = Terminal::new(80, 80, 0).unwrap();
    t.print('\u{200d}').unwrap();
    assert_eq!(
        (t.screens.active().cursor.x, t.screens.active().cursor.y),
        (0, 0)
    );
    assert!(!dirty(&t, 0));
}

#[test]
fn screen_zero_width_character_attaches_to_pending_wrap_cell() {
    let mut t = Terminal::new(2, 2, 0).unwrap();
    t.modes.set(Mode::GraphemeCluster, false);
    t.print_string("xå\u{332}").unwrap();
    assert_text(&t, "xå\u{332}");
}

#[test]
fn screen_printrepeat_simple() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.print_string("A").unwrap();
    t.print_repeat(1).unwrap();
    assert!(dirty(&t, 0));
    assert_text(&t, "AA");
}

#[test]
fn screen_printrepeat_wrap() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.print_string("    A").unwrap();
    t.print_repeat(1).unwrap();
    assert!(dirty(&t, 0));
    assert_text(&t, "    A\nA");
}

#[test]
fn screen_printrepeat_no_previous_character() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.print_repeat(1).unwrap();
    assert!(!dirty(&t, 0));
    assert_text(&t, "");
}

#[test]
fn screen_printslice_simple_ascii() {
    let mut t = Terminal::new(10, 3, 0).unwrap();
    t.print_slice(&['h', 'e', 'l', 'l', 'o']).unwrap();
    assert_eq!(t.screens.active().cursor.x, 5);
    assert_eq!(t.previous_char, Some('o'));
    assert!(dirty(&t, 0));
    assert_text(&t, "hello");
}

#[test]
fn screen_printslice_wraps_and_scrolls() {
    let mut t = Terminal::new(5, 2, 0).unwrap();
    t.print_slice(&['a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l'])
        .unwrap();
    assert_text(&t, "fghij\nkl");
    assert_eq!(t.screens.active().cursor.x, 2);
    assert!(!t.screens.active().cursor.pending_wrap);
}

#[test]
fn screen_printslice_pending_wrap_state() {
    let mut t = Terminal::new(5, 2, 0).unwrap();
    t.print_slice(&['a', 'b', 'c', 'd', 'e']).unwrap();
    assert_eq!(t.screens.active().cursor.x, 4);
    assert!(t.screens.active().cursor.pending_wrap);
    assert_text(&t, "abcde");
}

#[test]
fn screen_render_charset() {
    use maestro_terminal::{Charset, CharsetSlot as Slots};
    let mut t = Terminal::new(80, 80, 0).unwrap();
    for slot in [Slots::G1, Slots::G2, Slots::G3] {
        t.configure_charset(slot, Charset::DecSpecial);
    }
    assert!(!dirty(&t, 0));
    t.print('`').unwrap();
    for charset in [Charset::Utf8, Charset::Ascii, Charset::DecSpecial] {
        t.configure_charset(Slots::G0, charset);
        t.print('`').unwrap();
    }
    assert_text(&t, "```◆");
    assert!(dirty(&t, 0));
}

#[test]
fn screen_render_charset_outside_of_ascii() {
    use maestro_terminal::{Charset, CharsetSlot as Slots};
    let mut t = Terminal::new(80, 80, 0).unwrap();
    for slot in [Slots::G1, Slots::G2, Slots::G3] {
        t.configure_charset(slot, Charset::DecSpecial);
    }
    assert!(!dirty(&t, 0));
    t.configure_charset(Slots::G0, Charset::DecSpecial);
    t.print('`').unwrap();
    t.print('😀').unwrap();
    assert_text(&t, "◆ ");
    assert!(dirty(&t, 0));
}

#[test]
fn screen_render_invoke_charset() {
    use maestro_terminal::{Charset, CharsetActiveSlot as ActiveSlot, CharsetSlot as Slots};
    let mut t = Terminal::new(80, 80, 0).unwrap();
    t.configure_charset(Slots::G1, Charset::DecSpecial);
    t.print('`').unwrap();
    clear_dirty(&mut t);
    t.invoke_charset(ActiveSlot::Gl, Slots::G1, false);
    assert!(!dirty(&t, 0));
    t.print('`').unwrap();
    assert!(dirty(&t, 0));
    t.print('`').unwrap();
    t.invoke_charset(ActiveSlot::Gl, Slots::G0, false);
    t.print('`').unwrap();
    assert_text(&t, "`◆◆`");
}

#[test]
fn screen_render_invoke_charset_single() {
    use maestro_terminal::{Charset, CharsetActiveSlot as ActiveSlot, CharsetSlot as Slots};
    let mut t = Terminal::new(80, 80, 0).unwrap();
    t.configure_charset(Slots::G1, Charset::DecSpecial);
    t.print('`').unwrap();
    t.invoke_charset(ActiveSlot::Gl, Slots::G1, true);
    t.print('`').unwrap();
    t.print('`').unwrap();
    assert_text(&t, "`◆`");
}

#[test]
fn screen_render_kitty_unicode_placeholder() {
    let mut t = Terminal::new(10, 10, 0).unwrap();
    t.print('\u{10eeee}').unwrap();
    assert_eq!(
        (t.screens.active().cursor.x, t.screens.active().cursor.y),
        (1, 0)
    );
    assert_eq!(cell(&t, 0, 0).codepoint(), '\u{10eeee}');
    assert!(row(&t, 0).kitty_virtual_placeholder());
    assert!(dirty(&t, 0));
}

#[test]
fn screen_soft_wrap() {
    let mut t = Terminal::new(3, 80, 0).unwrap();
    t.print_string("hello").unwrap();
    assert_eq!(
        (t.screens.active().cursor.x, t.screens.active().cursor.y),
        (2, 1)
    );
    assert_text(&t, "hel\nlo");
}

#[test]
fn screen_disabled_wraparound_with_wide_char_and_one_space() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.modes.set(Mode::Wraparound, false);
    t.print_string("AAAA").unwrap();
    clear_dirty(&mut t);
    t.print('🚨').unwrap();
    assert_eq!(
        (t.screens.active().cursor.x, t.screens.active().cursor.y),
        (4, 0)
    );
    assert_text(&t, "AAAA");
    assert_eq!(
        (cell(&t, 4, 0).codepoint(), cell(&t, 4, 0).wide()),
        ('\0', Wide::Narrow)
    );
    assert!(!dirty(&t, 0));
}

#[test]
fn screen_disabled_wraparound_with_wide_char_and_no_space() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.modes.set(Mode::Wraparound, false);
    t.print_string("AAAAA").unwrap();
    clear_dirty(&mut t);
    t.print('🚨').unwrap();
    assert_eq!(
        (t.screens.active().cursor.x, t.screens.active().cursor.y),
        (4, 0)
    );
    assert_text(&t, "AAAAA");
    assert_eq!(
        (cell(&t, 4, 0).codepoint(), cell(&t, 4, 0).wide()),
        ('A', Wide::Narrow)
    );
    assert!(!dirty(&t, 0));
}

#[test]
fn screen_disabled_wraparound_with_wide_grapheme_and_half_space() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.modes.set(Mode::GraphemeCluster, true);
    t.modes.set(Mode::Wraparound, false);
    t.print_string("AAAA❤").unwrap();
    clear_dirty(&mut t);
    t.print('\u{fe0f}').unwrap();
    assert_eq!(
        (t.screens.active().cursor.x, t.screens.active().cursor.y),
        (4, 0)
    );
    assert_text(&t, "AAAA❤");
    assert_eq!(
        (cell(&t, 4, 0).codepoint(), cell(&t, 4, 0).wide()),
        ('❤', Wide::Narrow)
    );
    assert!(!dirty(&t, 0));
}

fn right_margin(t: &mut Terminal) {
    t.modes.set(Mode::EnableLeftAndRightMargin, true);
    t.scrolling_region.left = 2;
    t.scrolling_region.right = 4;
}

#[test]
fn screen_render_right_margin_wrap() {
    let mut t = Terminal::new(10, 5, 0).unwrap();
    t.print_string("123456789").unwrap();
    right_margin(&mut t);
    t.set_cursor_pos(1, 5);
    t.print_string("XY").unwrap();
    assert_text(&t, "1234X6789\n  Y");
    assert!(!row(&t, 0).wrap());
}

#[test]
fn screen_render_right_margin_wrap_dirty_tracking() {
    let mut t = Terminal::new(10, 5, 0).unwrap();
    t.print_string("123456789").unwrap();
    right_margin(&mut t);
    t.set_cursor_pos(1, 5);
    clear_dirty(&mut t);
    t.print('X').unwrap();
    assert!(dirty(&t, 0));
    assert!(!dirty(&t, 1));
    clear_dirty(&mut t);
    t.print('Y').unwrap();
    assert!(dirty(&t, 0));
    assert!(dirty(&t, 1));
    assert_text(&t, "1234X6789\n  Y");
}

#[test]
fn screen_render_right_margin_outside() {
    let mut t = Terminal::new(10, 5, 0).unwrap();
    t.print_string("123456789").unwrap();
    right_margin(&mut t);
    t.set_cursor_pos(1, 6);
    clear_dirty(&mut t);
    t.print_string("XY").unwrap();
    assert_text(&t, "12345XY89");
    assert!(dirty(&t, 0));
}

#[test]
fn screen_render_right_margin_outside_wrap() {
    let mut t = Terminal::new(10, 5, 0).unwrap();
    t.print_string("123456789").unwrap();
    right_margin(&mut t);
    t.set_cursor_pos(1, 10);
    t.print_string("XY").unwrap();
    assert_text(&t, "123456789X\n  Y");
}

#[test]
fn screen_render_wide_char_at_right_margin_does_not_create_spacer_head() {
    let mut t = Terminal::new(10, 10, 0).unwrap();
    right_margin(&mut t);
    t.set_cursor_pos(1, 5);
    t.print('😀').unwrap();
    assert_eq!(
        (t.screens.active().cursor.x, t.screens.active().cursor.y),
        (4, 1)
    );
    assert!(dirty(&t, 0));
    assert!(dirty(&t, 1));
    assert_eq!(
        (cell(&t, 4, 0).codepoint(), cell(&t, 4, 0).wide()),
        ('\0', Wide::Narrow)
    );
    assert!(!row(&t, 0).wrap());
    assert_eq!(
        (cell(&t, 2, 1).codepoint(), cell(&t, 2, 1).wide()),
        ('😀', Wide::Wide)
    );
    assert_eq!(cell(&t, 3, 1).wide(), Wide::SpacerTail);
}

#[test]
fn screen_render_wide_char_at_right_edge_with_hyperlink() {
    let mut t = Terminal::new(10, 5, 0).unwrap();
    t.screens
        .active_mut()
        .start_hyperlink(b"http://example.com", None)
        .unwrap();
    t.set_cursor_pos(1, 10);
    t.print('中').unwrap();
    assert_eq!(
        (t.screens.active().cursor.x, t.screens.active().cursor.y),
        (2, 1)
    );
    assert_eq!(cell(&t, 9, 0).wide(), Wide::SpacerHead);
    assert!(cell(&t, 9, 0).hyperlink());
    assert!(row(&t, 0).wrap());
    assert_eq!(
        (cell(&t, 0, 1).codepoint(), cell(&t, 0, 1).wide()),
        ('中', Wide::Wide)
    );
    assert!(cell(&t, 0, 1).hyperlink());
    assert_eq!(cell(&t, 1, 1).wide(), Wide::SpacerTail);
    assert!(cell(&t, 1, 1).hyperlink());
}

#[test]
fn terminal_print_entry_points_share_cells_and_dirty_rows() {
    let mut scalar = Terminal::new(5, 3, 0).unwrap();
    let mut slice = Terminal::new(5, 3, 0).unwrap();
    let mut utf8 = Terminal::new(5, 3, 0).unwrap();
    let input = "a\u{301}界xy❤\u{fe0f}";
    for cp in input.chars() {
        scalar.print(cp).unwrap();
    }
    slice
        .print_slice(&input.chars().collect::<Vec<_>>())
        .unwrap();
    utf8.print_string(input).unwrap();
    assert_printed_equal(&scalar, &slice);
    assert_printed_equal(&scalar, &utf8);
    scalar.carriage_return();
    scalar.linefeed().unwrap();
    scalar.print('z').unwrap();
    utf8.print_string("\nz").unwrap();
    assert_printed_equal(&scalar, &utf8);
}

fn assert_printed_equal(left: &Terminal, right: &Terminal) {
    use super::support::terminal::suffix;
    let a = &left.screens.active().cursor;
    let b = &right.screens.active().cursor;
    assert_eq!((a.x, a.y, a.pending_wrap), (b.x, b.y, b.pending_wrap));
    assert_eq!(left.previous_char, right.previous_char);
    for y in 0..u32::from(left.rows()) {
        assert_eq!(dirty(left, y), dirty(right, y));
        for x in 0..left.cols() {
            assert_eq!(cell(left, x, y), cell(right, x, y));
            assert_eq!(suffix(left, x, y), suffix(right, x, y));
        }
    }
}

#[test]
fn screen_render_writes_to_bottom_if_scrolled() {
    use maestro_terminal::terminal::ScrollViewport;
    let mut t = Terminal::new(5, 2, 10_000_000).unwrap();
    t.print_string("hello").unwrap();
    t.set_cursor_pos(0, 0);
    for _ in 0..3 {
        t.index().unwrap();
    }
    assert_text(&t, "");
    t.scroll_viewport(ScrollViewport::Top);
    let screen = t.screens.active();
    let pin = screen
        .pages
        .pin(Point::Viewport(Coordinate { x: 0, y: 0 }))
        .unwrap();
    let page = screen.pages.page(pin.node).unwrap();
    for (value, expected) in page.get_cells(pin.y).iter().zip("hello".chars()) {
        assert_eq!(value.codepoint(), expected);
    }
    t.print('A').unwrap();
    t.scroll_viewport(ScrollViewport::Bottom);
    assert_text(&t, "\nA");
    assert!(dirty(&t, u32::from(t.screens.active().cursor.y)));
}
