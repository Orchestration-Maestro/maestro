//! Cursor moves with retained printing and wrap observations.

use super::support::terminal::assert_text;
use maestro_terminal::{Mode, Terminal};

#[test]
fn cursor_state_cursorup_basic() {
    let mut t = Terminal::new(5, 5, 0).unwrap();

    t.set_cursor_pos(3, 1);
    t.print('A').unwrap();
    t.cursor_up(10);
    t.print('X').unwrap();

    {
        assert_text(&t, " X\n\nA");
    }
}

#[test]
fn cursor_state_cursorup_below_top_scroll_margin() {
    let mut t = Terminal::new(5, 5, 0).unwrap();

    t.scrolling_region.top = 1;
    t.scrolling_region.bottom = 3;
    t.set_cursor_pos(1, 1);
    t.set_cursor_pos(3, 1);
    t.print('A').unwrap();
    t.cursor_up(5);
    t.print('X').unwrap();

    {
        assert_text(&t, "\n X\nA");
    }
}

#[test]
fn cursor_state_cursorup_above_top_scroll_margin() {
    let mut t = Terminal::new(5, 5, 0).unwrap();

    t.scrolling_region.top = 2;
    t.scrolling_region.bottom = 4;
    t.set_cursor_pos(1, 1);
    t.set_cursor_pos(3, 1);
    t.print('A').unwrap();
    t.set_cursor_pos(2, 1);
    t.cursor_up(10);
    t.print('X').unwrap();

    {
        assert_text(&t, "X\n\nA");
    }
}

#[test]
fn cursor_state_cursorup_resets_wrap() {
    let mut t = Terminal::new(5, 5, 0).unwrap();

    t.print_string("ABCDE").unwrap();
    assert!(t.screens.active().cursor.pending_wrap);
    t.cursor_up(1);
    assert!(!t.screens.active().cursor.pending_wrap);
    t.print('X').unwrap();

    {
        assert_text(&t, "ABCDX");
    }
}

#[test]
fn cursor_state_cursorleft_no_wrap() {
    let mut t = Terminal::new(10, 5, 0).unwrap();

    t.print('A').unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print('B').unwrap();
    t.cursor_left(10);

    {
        assert_text(&t, "A\nB");
    }
}

#[test]
fn cursor_state_cursorleft_unsets_pending_wrap_state() {
    let mut t = Terminal::new(5, 5, 0).unwrap();

    t.print_string("ABCDE").unwrap();
    assert!(t.screens.active().cursor.pending_wrap);
    t.cursor_left(1);
    assert!(!t.screens.active().cursor.pending_wrap);
    t.print('X').unwrap();

    {
        assert_text(&t, "ABCXE");
    }
}

#[test]
fn cursor_state_cursorleft_unsets_pending_wrap_state_with_longer_jump() {
    let mut t = Terminal::new(5, 5, 0).unwrap();

    t.print_string("ABCDE").unwrap();
    assert!(t.screens.active().cursor.pending_wrap);
    t.cursor_left(3);
    assert!(!t.screens.active().cursor.pending_wrap);
    t.print('X').unwrap();

    {
        assert_text(&t, "AXCDE");
    }
}

#[test]
fn cursor_state_cursorleft_reverse_wrap_with_pending_wrap_state() {
    let mut t = Terminal::new(5, 5, 0).unwrap();

    t.modes.set(Mode::Wraparound, true);
    t.modes.set(Mode::ReverseWrap, true);

    t.print_string("ABCDE").unwrap();
    assert!(t.screens.active().cursor.pending_wrap);
    t.cursor_left(1);
    assert!(!t.screens.active().cursor.pending_wrap);
    t.print('X').unwrap();

    {
        assert_text(&t, "ABCDX");
    }
}

#[test]
fn cursor_state_cursorleft_reverse_wrap_extended_with_pending_wrap_state() {
    let mut t = Terminal::new(5, 5, 0).unwrap();

    t.modes.set(Mode::Wraparound, true);
    t.modes.set(Mode::ReverseWrapExtended, true);

    t.print_string("ABCDE").unwrap();
    assert!(t.screens.active().cursor.pending_wrap);
    t.cursor_left(1);
    assert!(!t.screens.active().cursor.pending_wrap);
    t.print('X').unwrap();

    {
        assert_text(&t, "ABCDX");
    }
}

#[test]
fn cursor_state_cursorleft_reverse_wrap() {
    let mut t = Terminal::new(5, 5, 0).unwrap();

    t.modes.set(Mode::Wraparound, true);
    t.modes.set(Mode::ReverseWrap, true);

    t.print_string("ABCDE1").unwrap();
    t.cursor_left(2);
    t.print('X').unwrap();
    assert!(t.screens.active().cursor.pending_wrap);

    {
        assert_text(&t, "ABCDX\n1");
    }
}

#[test]
fn cursor_state_cursorleft_reverse_wrap_with_no_soft_wrap() {
    let mut t = Terminal::new(5, 5, 0).unwrap();

    t.modes.set(Mode::Wraparound, true);
    t.modes.set(Mode::ReverseWrap, true);

    t.print_string("ABCDE").unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print('1').unwrap();
    t.cursor_left(2);
    t.print('X').unwrap();

    {
        assert_text(&t, "ABCDE\nX");
    }
}

#[test]
fn cursor_state_cursorleft_reverse_wrap_before_left_margin() {
    let mut t = Terminal::new(5, 5, 0).unwrap();

    t.modes.set(Mode::Wraparound, true);
    t.modes.set(Mode::ReverseWrap, true);
    t.scrolling_region.top = 2;
    t.scrolling_region.bottom = 4;
    t.set_cursor_pos(1, 1);
    t.cursor_left(1);
    t.print('X').unwrap();

    {
        assert_text(&t, "\n\nX");
    }
}

#[test]
fn cursor_state_cursorleft_extended_reverse_wrap() {
    let mut t = Terminal::new(5, 5, 0).unwrap();

    t.modes.set(Mode::Wraparound, true);
    t.modes.set(Mode::ReverseWrapExtended, true);

    t.print_string("ABCDE").unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print('1').unwrap();
    t.cursor_left(2);
    t.print('X').unwrap();

    {
        assert_text(&t, "ABCDX\n1");
    }
}

#[test]
fn cursor_state_cursorleft_extended_reverse_wrap_bottom_wraparound() {
    let mut t = Terminal::new(5, 3, 0).unwrap();

    t.modes.set(Mode::Wraparound, true);
    t.modes.set(Mode::ReverseWrapExtended, true);

    t.print_string("ABCDE").unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print('1').unwrap();
    t.cursor_left(1 + usize::from(t.cols()) + 1);
    t.print('X').unwrap();

    {
        assert_text(&t, "ABCDE\n1\n    X");
    }
}

#[test]
fn cursor_state_cursorleft_extended_reverse_wrap_is_priority_if_both_set() {
    let mut t = Terminal::new(5, 3, 0).unwrap();

    t.modes.set(Mode::Wraparound, true);
    t.modes.set(Mode::ReverseWrap, true);
    t.modes.set(Mode::ReverseWrapExtended, true);

    t.print_string("ABCDE").unwrap();
    t.carriage_return();
    t.linefeed().unwrap();
    t.print('1').unwrap();
    t.cursor_left(1 + usize::from(t.cols()) + 1);
    t.print('X').unwrap();

    {
        assert_text(&t, "ABCDE\n1\n    X");
    }
}

#[test]
fn cursor_state_cursorleft_extended_reverse_wrap_above_top_scroll_region() {
    let mut t = Terminal::new(5, 5, 0).unwrap();

    t.modes.set(Mode::Wraparound, true);
    t.modes.set(Mode::ReverseWrapExtended, true);

    t.scrolling_region.top = 2;
    t.scrolling_region.bottom = 4;
    t.set_cursor_pos(1, 1);
    t.set_cursor_pos(2, 1);
    t.cursor_left(1000);

    assert_eq!(t.screens.active().cursor.x, 0);
    assert_eq!(t.screens.active().cursor.y, 0);
}

#[test]
fn cursor_state_cursorleft_reverse_wrap_on_first_row() {
    let mut t = Terminal::new(5, 5, 0).unwrap();

    t.modes.set(Mode::Wraparound, true);
    t.modes.set(Mode::ReverseWrap, true);

    t.scrolling_region.top = 2;
    t.scrolling_region.bottom = 4;
    t.set_cursor_pos(1, 1);
    t.set_cursor_pos(1, 2);
    t.cursor_left(1000);

    assert_eq!(t.screens.active().cursor.x, 0);
    assert_eq!(t.screens.active().cursor.y, 0);
}

#[test]
fn cursor_state_cursordown_basic() {
    let mut t = Terminal::new(5, 5, 0).unwrap();

    t.print('A').unwrap();
    t.cursor_down(10);
    t.print('X').unwrap();

    {
        assert_text(&t, "A\n\n\n\n X");
    }
}

#[test]
fn cursor_state_cursordown_above_bottom_scroll_margin() {
    let mut t = Terminal::new(5, 5, 0).unwrap();

    t.scrolling_region.top = 0;
    t.scrolling_region.bottom = 2;
    t.set_cursor_pos(1, 1);
    t.print('A').unwrap();
    t.cursor_down(10);
    t.print('X').unwrap();

    {
        assert_text(&t, "A\n\n X");
    }
}

#[test]
fn cursor_state_cursordown_below_bottom_scroll_margin() {
    let mut t = Terminal::new(5, 5, 0).unwrap();

    t.scrolling_region.top = 0;
    t.scrolling_region.bottom = 2;
    t.set_cursor_pos(1, 1);
    t.print('A').unwrap();
    t.set_cursor_pos(4, 1);
    t.cursor_down(10);
    t.print('X').unwrap();

    {
        assert_text(&t, "A\n\n\n\nX");
    }
}

#[test]
fn cursor_state_cursordown_resets_wrap() {
    let mut t = Terminal::new(5, 5, 0).unwrap();

    t.print_string("ABCDE").unwrap();
    assert!(t.screens.active().cursor.pending_wrap);
    t.cursor_down(1);
    assert!(!t.screens.active().cursor.pending_wrap);
    t.print('X').unwrap();

    {
        assert_text(&t, "ABCDE\n    X");
    }
}

#[test]
fn cursor_state_cursorright_resets_wrap() {
    let mut t = Terminal::new(5, 5, 0).unwrap();

    t.print_string("ABCDE").unwrap();
    assert!(t.screens.active().cursor.pending_wrap);
    t.cursor_right(1);
    assert!(!t.screens.active().cursor.pending_wrap);
    t.print('X').unwrap();

    {
        assert_text(&t, "ABCDX");
    }
}

#[test]
fn cursor_state_cursorright_left_of_right_margin() {
    let mut t = Terminal::new(5, 5, 0).unwrap();

    t.scrolling_region.right = 2;
    t.cursor_right(100);
    t.print('X').unwrap();

    {
        assert_text(&t, "  X");
    }
}

#[test]
fn cursor_state_cursorright_right_of_right_margin() {
    let mut t = Terminal::new(5, 5, 0).unwrap();

    t.scrolling_region.right = 2;
    t.set_cursor_pos(1, 4);
    t.cursor_right(100);
    t.print('X').unwrap();

    {
        assert_text(&t, "    X");
    }
}

#[test]
fn cursor_state_cursorpos_resets_wrap() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.print_string("ABCDE").unwrap();
    assert!(t.screens.active().cursor.pending_wrap);
    t.set_cursor_pos(1, 1);
    assert!(!t.screens.active().cursor.pending_wrap);
    t.print('X').unwrap();
    assert_text(&t, "XBCDE");
}

#[test]
fn cursor_state_cursorpos_relative_to_origin() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.scrolling_region.top = 2;
    t.scrolling_region.bottom = 3;
    t.modes.set(Mode::Origin, true);
    t.set_cursor_pos(1, 1);
    t.print('X').unwrap();
    assert_text(&t, "\n\nX");
}

#[test]
fn cursor_state_cursorpos_relative_to_origin_with_left_right() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.scrolling_region.top = 2;
    t.scrolling_region.bottom = 3;
    t.scrolling_region.left = 2;
    t.scrolling_region.right = 4;
    t.modes.set(Mode::Origin, true);
    t.set_cursor_pos(1, 1);
    t.print('X').unwrap();
    assert_text(&t, "\n\n  X");
}

#[test]
fn cursor_state_cursorpos_limits_with_full_scroll_region() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.scrolling_region.top = 2;
    t.scrolling_region.bottom = 3;
    t.scrolling_region.left = 2;
    t.scrolling_region.right = 4;
    t.modes.set(Mode::Origin, true);
    t.set_cursor_pos(500, 500);
    t.print('X').unwrap();
    assert_text(&t, "\n\n\n    X");
}

#[test]
fn cursor_state_setcursorpos_original_test() {
    let mut t = Terminal::new(80, 80, 0).unwrap();
    assert_eq!(
        (t.screens.active().cursor.x, t.screens.active().cursor.y),
        (0, 0)
    );
    t.set_cursor_pos(0, 0);
    assert_eq!(
        (t.screens.active().cursor.x, t.screens.active().cursor.y),
        (0, 0)
    );
    t.set_cursor_pos(81, 81);
    assert_eq!(
        (t.screens.active().cursor.x, t.screens.active().cursor.y),
        (79, 79)
    );
    t.set_cursor_pos(0, 80);
    t.print('c').unwrap();
    assert!(t.screens.active().cursor.pending_wrap);
    t.set_cursor_pos(0, 80);
    assert!(!t.screens.active().cursor.pending_wrap);
    t.modes.set(Mode::Origin, true);
    t.set_cursor_pos(81, 81);
    assert_eq!(
        (t.screens.active().cursor.x, t.screens.active().cursor.y),
        (79, 79)
    );
    t.scrolling_region.top = 9;
    t.scrolling_region.bottom = 79;
    t.set_cursor_pos(0, 0);
    assert_eq!(
        (t.screens.active().cursor.x, t.screens.active().cursor.y),
        (0, 9)
    );
    t.set_cursor_pos(1, 1);
    assert_eq!(
        (t.screens.active().cursor.x, t.screens.active().cursor.y),
        (0, 9)
    );
    t.set_cursor_pos(100, 0);
    assert_eq!(
        (t.screens.active().cursor.x, t.screens.active().cursor.y),
        (0, 79)
    );
    t.scrolling_region.bottom = 10;
    t.set_cursor_pos(2, 0);
    assert_eq!(
        (t.screens.active().cursor.x, t.screens.active().cursor.y),
        (0, 10)
    );
}

#[test]
fn cursor_state_savecursor() {
    use maestro_terminal::{CharsetSlot as Slots, sgr::Attribute};
    let mut t = Terminal::new(3, 3, 0).unwrap();
    t.set_attribute(Attribute::Bold).unwrap();
    t.screens.active_mut().charset.gr = Slots::G3;
    t.modes.set(Mode::Origin, true);
    t.save_cursor();
    t.screens.active_mut().charset.gr = Slots::G0;
    t.set_attribute(Attribute::Unset).unwrap();
    t.modes.set(Mode::Origin, false);
    t.restore_cursor();
    assert!(t.screens.active().cursor.style.flags.bold());
    assert_eq!(t.screens.active().charset.gr, Slots::G3);
    assert!(t.modes.get(Mode::Origin));
}

#[test]
fn cursor_state_savecursor_position() {
    let mut t = Terminal::new(10, 5, 0).unwrap();
    t.set_cursor_pos(1, 5);
    t.print('A').unwrap();
    t.save_cursor();
    t.set_cursor_pos(1, 1);
    t.print('B').unwrap();
    t.restore_cursor();
    t.print('X').unwrap();
    assert_text(&t, "B   AX");
}

#[test]
fn cursor_state_savecursor_pending_wrap_state() {
    let mut t = Terminal::new(5, 5, 0).unwrap();
    t.set_cursor_pos(1, 5);
    t.print('A').unwrap();
    t.save_cursor();
    t.set_cursor_pos(1, 1);
    t.print('B').unwrap();
    t.restore_cursor();
    t.print('X').unwrap();
    assert_text(&t, "B   A\nX");
}

#[test]
fn cursor_state_savecursor_origin_mode() {
    let mut t = Terminal::new(10, 5, 0).unwrap();
    t.modes.set(Mode::Origin, true);
    t.save_cursor();
    t.modes.set(Mode::EnableLeftAndRightMargin, true);
    t.scrolling_region.left = 2;
    t.scrolling_region.right = 4;
    t.scrolling_region.top = 1;
    t.scrolling_region.bottom = 3;
    t.set_cursor_pos(1, 1);
    t.restore_cursor();
    t.print('X').unwrap();
    assert_text(&t, "X");
}

#[test]
fn cursor_state_savecursor_does_not_modify_hyperlink_state() {
    let mut t = Terminal::new(3, 3, 0).unwrap();
    t.screens
        .active_mut()
        .start_hyperlink(b"http://example.com", None)
        .unwrap();
    let id = t.screens.active().cursor.hyperlink_id;
    t.save_cursor();
    assert_eq!(t.screens.active().cursor.hyperlink_id, id);
    t.restore_cursor();
    assert_eq!(t.screens.active().cursor.hyperlink_id, id);
}
