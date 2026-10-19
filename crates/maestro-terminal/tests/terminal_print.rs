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
