//! Byte input applied to retained terminal screens.

use super::support::terminal::{assert_text, cell};
use maestro_terminal::Terminal;

#[test]
fn terminal_bytes_basic_print() {
    let mut t = Terminal::new(10, 3, 0).unwrap();
    t.write("hello\r\n界!".as_bytes());
    assert_text(&t, "hello\n界!");
    assert_eq!(
        (t.screens.active().cursor.x, t.screens.active().cursor.y),
        (3, 1)
    );
    assert_eq!(cell(&t, 0, 1).codepoint(), '界');
}

fn position(t: &Terminal) -> (u16, u16) {
    let c = &t.screens.active().cursor;
    (c.x, c.y)
}

#[test]
fn terminal_bytes_cursor_movement() {
    let mut t = Terminal::new(10, 10, 0).unwrap();
    t.write(b"Hello\x1b[1;1H");
    assert_eq!(position(&t), (0, 0));
    t.write(b"\x1b[2;3H");
    assert_eq!(position(&t), (2, 1));
}

#[test]
fn terminal_bytes_erase_operations() {
    let mut t = Terminal::new(20, 10, 0).unwrap();
    t.write(b"Hello World");
    assert_eq!(position(&t), (11, 0));
    t.write(b"\x1b[1;6H\x1b[K");
    assert_text(&t, "Hello");
}

#[test]
fn terminal_bytes_tabs() {
    let mut t = Terminal::new(80, 10, 0).unwrap();
    t.write(b"A\tB");
    assert_eq!(position(&t), (9, 0));
    assert_text(&t, "A       B");
}

#[test]
fn terminal_bytes_modes() {
    use maestro_terminal::Mode;
    let mut t = Terminal::new(80, 24, 0).unwrap();
    assert!(t.modes.get(Mode::Wraparound));
    t.write(b"\x1b[?7l");
    assert!(!t.modes.get(Mode::Wraparound));
    t.write(b"\x1b[?7h");
    assert!(t.modes.get(Mode::Wraparound));
}

#[test]
fn terminal_bytes_scrolling_regions() {
    let mut t = Terminal::new(80, 24, 0).unwrap();
    t.write(b"\x1b[5;20r");
    let r = t.scrolling_region;
    assert_eq!((r.top, r.bottom, r.left, r.right), (4, 19, 0, 79));
}

#[test]
fn terminal_bytes_charsets() {
    let mut t = Terminal::new(80, 24, 0).unwrap();
    t.write(b"\x1b(0`");
    assert_text(&t, "◆");
}

#[test]
fn terminal_bytes_alt_screen() {
    use maestro_terminal::screen_set::Key;
    let mut t = Terminal::new(10, 5, 0).unwrap();
    t.write(b"Primary");
    assert_eq!(t.active_screen(), Key::Primary);
    t.write(b"\x1b[?1049hAlt");
    assert_eq!(t.active_screen(), Key::Alternate);
    assert_text(&t, "       Alt");
    t.write(b"\x1b[?1049l");
    assert_eq!(t.active_screen(), Key::Primary);
    assert_text(&t, "Primary");
}

#[test]
fn terminal_bytes_cursor_save_and_restore() {
    let mut t = Terminal::new(80, 24, 0).unwrap();
    t.write(b"\x1b[10;15H\x1b7");
    assert_eq!(position(&t), (14, 9));
    t.write(b"\x1b[1;1H");
    assert_eq!(position(&t), (0, 0));
    t.write(b"\x1b8");
    assert_eq!(position(&t), (14, 9));
}

#[test]
fn terminal_bytes_attributes() {
    let mut t = Terminal::new(80, 24, 0).unwrap();
    t.write(b"\x1b[1mBold");
    assert!(t.screens.active().cursor.style.flags.bold());
    t.write(b"\x1b[0m");
    assert!(!t.screens.active().cursor.style.flags.bold());
    assert_text(&t, "Bold");
}

#[test]
fn terminal_bytes_decaln_screen_alignment() {
    let mut t = Terminal::new(10, 3, 0).unwrap();
    t.write(b"\x1b#8");
    assert_text(&t, "EEEEEEEEEE\nEEEEEEEEEE\nEEEEEEEEEE");
    assert_eq!(position(&t), (0, 0));
}

#[test]
fn terminal_bytes_full_reset() {
    use maestro_terminal::{Mode, osc::semantic_prompt::Redraw};
    let mut t = Terminal::new(80, 24, 0).unwrap();
    t.write(b"Hello\x1b[10;20H\x1b[5;20r\x1b[?7l\x1b]133;A;redraw=1\x07");
    t.write(b"\x1bc");
    assert_eq!(position(&t), (0, 0));
    assert_eq!((t.scrolling_region.top, t.scrolling_region.bottom), (0, 23));
    assert!(t.modes.get(Mode::Wraparound));
    assert!(t.modes.get(Mode::GraphemeCluster));
    assert_eq!(t.flags.shell_redraws_prompt, Redraw::False);
    assert_text(&t, "");
}

#[test]
fn terminal_bytes_semantic_prompt_fresh_line() {
    let mut t = Terminal::new(10, 10, 0).unwrap();
    t.write(b"Hello\x1b]133;L\x07");
    assert_eq!(position(&t), (0, 1));
}

#[test]
fn terminal_bytes_semantic_prompt_fresh_line_new_prompt() {
    use maestro_terminal::{osc::semantic_prompt::Redraw, page::cell::SemanticContent};
    let mut t = Terminal::new(10, 10, 0).unwrap();
    t.write(b"Hello\x1b]133;A\x07");
    assert_eq!(position(&t), (0, 1));
    assert_eq!(
        t.screens.active().cursor.semantic_content,
        SemanticContent::Prompt
    );
    t.write(b"prompt$ \x1b]133;A;redraw=1\x07");
    assert_eq!(t.flags.shell_redraws_prompt, Redraw::True);
}

#[test]
fn terminal_bytes_semantic_prompt_end_of_input_then_start_output() {
    use maestro_terminal::page::cell::SemanticContent;
    let mut t = Terminal::new(10, 10, 0).unwrap();
    t.write(b"Hello\x1b]133;A\x07prompt$ \x1b]133;B\x07");
    assert_eq!(
        t.screens.active().cursor.semantic_content,
        SemanticContent::Input
    );
    t.write(b"\x1b]133;C\x07");
    assert_eq!(
        t.screens.active().cursor.semantic_content,
        SemanticContent::Output
    );
}

#[test]
fn terminal_bytes_semantic_prompt_prompt_start() {
    use maestro_terminal::page::cell::SemanticContent;
    let mut t = Terminal::new(10, 10, 0).unwrap();
    t.write(b"Hello\x1b]133;P\x07");
    assert_eq!(
        t.screens.active().cursor.semantic_content,
        SemanticContent::Prompt
    );
    assert_eq!(position(&t), (5, 0));
}

#[test]
fn terminal_bytes_semantic_prompt_new_command() {
    use maestro_terminal::page::cell::SemanticContent;
    let mut t = Terminal::new(10, 10, 0).unwrap();
    t.write(b"Hello\x1b]133;N\x07");
    assert_eq!(position(&t), (0, 1));
    assert_eq!(
        t.screens.active().cursor.semantic_content,
        SemanticContent::Prompt
    );
}

#[test]
fn terminal_bytes_semantic_prompt_new_command_at_column_zero() {
    use maestro_terminal::page::cell::SemanticContent;
    let mut t = Terminal::new(10, 10, 0).unwrap();
    t.write(b"\x1b]133;N\x07");
    assert_eq!(position(&t), (0, 0));
    assert_eq!(
        t.screens.active().cursor.semantic_content,
        SemanticContent::Prompt
    );
}

#[test]
fn terminal_bytes_semantic_prompt_end_prompt_start_input_terminate_eol_clears_on_linefeed() {
    use maestro_terminal::page::cell::SemanticContent;
    let mut t = Terminal::new(10, 10, 0).unwrap();
    t.write(b"\x1b]133;I\x07");
    assert_eq!(
        t.screens.active().cursor.semantic_content,
        SemanticContent::Input
    );
    t.write(b"\n");
    assert_eq!(
        t.screens.active().cursor.semantic_content,
        SemanticContent::Output
    );
}

#[test]
fn terminal_bytes_csi_w_with_intermediate_but_no_params() {
    let mut t = Terminal::new(20, 3, 0).unwrap();
    t.write(b"\x1b[4G\x1b[?W\r\t");
    assert_eq!(position(&t), (8, 0));
}

#[test]
fn single_axis_handler_translates_origin_once() {
    use maestro_terminal::{
        stream::{Action, CursorAction, Handler},
        stream_terminal,
    };
    for (action, expected) in [
        (CursorAction::CursorCol(4), (6, 3)),
        (CursorAction::CursorRow(4), (4, 5)),
        (CursorAction::CursorColRelative(2), (6, 3)),
        (CursorAction::CursorRowRelative(2), (4, 5)),
    ] {
        let mut t = origin_terminal();
        stream_terminal::Handler::new(&mut t).vt(Action::Cursor(action));
        assert_eq!(position(&t), expected);
        t.write(b"!");
        assert_eq!(cell(&t, expected.0, u32::from(expected.1)).codepoint(), '!');
    }
    for (x, y, action, expected) in [
        (0, 0, CursorAction::CursorCol(4), (6, 2)),
        (11, 9, CursorAction::CursorRow(4), (8, 5)),
        (0, 3, CursorAction::CursorColRelative(2), (3, 3)),
        (4, 0, CursorAction::CursorRowRelative(1), (4, 2)),
    ] {
        let mut t = origin_terminal();
        t.screens.active_mut().cursor_absolute(x, y);
        stream_terminal::Handler::new(&mut t).vt(Action::Cursor(action));
        assert_eq!(position(&t), expected);
    }
}

fn origin_terminal() -> Terminal {
    let mut t = Terminal::new(12, 10, 0).unwrap();
    t.write(b"\x1b[3;8r\x1b[?69h\x1b[4;9s\x1b[?6h\x1b[2;2H");
    assert_eq!(position(&t), (4, 3));
    t
}

#[test]
fn single_axis_bytes_do_not_apply_origin_twice() {
    for (bytes, expected) in [
        (b"\x1b[4G".as_slice(), (6, 3)),
        (b"\x1b[4d", (4, 5)),
        (b"\x1b[2a", (6, 3)),
        (b"\x1b[2e", (4, 5)),
    ] {
        let mut t = origin_terminal();
        t.write(bytes);
        assert_eq!(position(&t), expected);
        t.write(b"!");
        assert_eq!(cell(&t, expected.0, u32::from(expected.1)).codepoint(), '!');
    }
}

#[test]
fn single_axis_origin_off_and_zero_margins_are_controls() {
    for origin in [false, true] {
        for (bytes, expected) in [
            (b"\x1b[4G".as_slice(), (3, 1)),
            (b"\x1b[4d", (1, 3)),
            (b"\x1b[2a", (3, 1)),
            (b"\x1b[2e", (1, 3)),
        ] {
            let mut t = Terminal::new(12, 10, 0).unwrap();
            if origin {
                t.write(b"\x1b[?6h");
            }
            t.write(b"\x1b[2;2H");
            t.write(bytes);
            assert_eq!(position(&t), expected);
        }
    }
}

#[test]
fn default_tab_clear_changes_the_next_destination() {
    for clear in [b"\x1b[g".as_slice(), b"\x1b[0g"] {
        let mut t = Terminal::new(20, 3, 0).unwrap();
        t.write(b"\t");
        assert_eq!(position(&t), (8, 0));
        t.write(clear);
        t.write(b"\r\t");
        assert_eq!(position(&t), (16, 0));
    }
}

#[test]
fn relative_zero_counts_advance_one_axis() {
    for origin in [false, true] {
        for (bytes, delta) in [(b"\x1b[0a".as_slice(), (1, 0)), (b"\x1b[0e", (0, 1))] {
            let mut t = if origin {
                origin_terminal()
            } else {
                let mut t = Terminal::new(12, 10, 0).unwrap();
                t.write(b"\x1b[2;2H");
                t
            };
            let before = position(&t);
            t.write(bytes);
            assert_eq!(position(&t), (before.0 + delta.0, before.1 + delta.1));
        }
    }
}

#[test]
fn cursor_style_requests_retain_default_state() {
    use maestro_terminal::{
        CursorStyle as S, Mode,
        stream::{Action, Handler, Stream},
        stream_terminal,
    };
    let mut t = Terminal::new(10, 3, 0).unwrap();
    let mut h = stream_terminal::Handler::new(&mut t);
    h.default_cursor_style = S::Bar;
    h.default_cursor_blink = true;
    for (n, shape, blink) in [
        (0, S::Bar, true),
        (1, S::Block, true),
        (2, S::Block, false),
        (3, S::Underline, true),
        (4, S::Underline, false),
        (5, S::Bar, true),
        (6, S::Bar, false),
    ] {
        Stream::new(false).next_slice(format!("\x1b[{n} q").as_bytes(), &mut h);
        assert_eq!(h.terminal.screens.active().cursor.cursor_style, shape);
        assert_eq!(h.terminal.modes.get(Mode::CursorBlinking), blink);
        assert_eq!(h.default_cursor, n == 0);
    }
    h.vt(Action::FullReset);
    assert!(h.default_cursor);
    assert_eq!(h.terminal.screens.active().cursor.cursor_style, S::Bar);
    assert!(h.terminal.modes.get(Mode::CursorBlinking));
    t.write(b"\x1b[3 q");
    t.write(b"");
    assert_eq!(t.screens.active().cursor.cursor_style, S::Underline);
    t.write(b"\x1b[ q");
    assert_eq!(t.screens.active().cursor.cursor_style, S::Block);
    t.write(b"\x1b[5 q\x1bc");
    assert_eq!(t.screens.active().cursor.cursor_style, S::Block);
    assert!(!t.modes.get(Mode::CursorBlinking));
}

#[test]
fn mouse_modes_keep_independent_flags_and_reads() {
    use maestro_terminal::{
        Mode,
        mouse::{Event, Format},
    };
    let mut t = Terminal::new(10, 3, 0).unwrap();
    for (n, event) in [
        (9, Event::X10),
        (1000, Event::Normal),
        (1002, Event::Button),
        (1003, Event::Any),
    ] {
        t.write(format!("\x1b[?{n}h").as_bytes());
        assert_eq!(t.flags.mouse_event, event);
        assert!(t.mouse_tracking_enabled());
        t.write(b"\x1b[?1006h");
        assert_eq!(t.flags.mouse_format, Format::Sgr);
        t.write(format!("\x1b[?{n}l").as_bytes());
        assert_eq!(t.flags.mouse_event, Event::None);
        assert!(!t.mouse_tracking_enabled());
        assert_eq!(t.flags.mouse_format, Format::Sgr);
    }
    t.write(b"\x1b[?9;1000h\x1b[?9l");
    assert!(t.modes.get(Mode::MouseEventNormal));
    assert_eq!(t.flags.mouse_event, Event::None);
    assert!(t.mouse_tracking_enabled());
    mouse_format_reads(&mut t);
}

fn mouse_format_reads(t: &mut Terminal) {
    use maestro_terminal::mouse::{Event, Format};
    for (n, format) in [
        (1005, Format::Utf8),
        (1006, Format::Sgr),
        (1015, Format::Urxvt),
        (1016, Format::SgrPixels),
    ] {
        t.write(format!("\x1b[?{n}h").as_bytes());
        assert_eq!(t.flags.mouse_format, format);
        assert_eq!(t.flags.mouse_event, Event::None);
        t.write(b"\x1b[?1002h");
        t.write(format!("\x1b[?{n}l").as_bytes());
        assert_eq!(t.flags.mouse_format, Format::X10);
        assert_eq!(t.flags.mouse_event, Event::Button);
        t.write(b"\x1b[?1002l");
    }
}

#[test]
fn keyboard_operations_reuse_screen_local_history() {
    let mut t = Terminal::new(10, 3, 0).unwrap();
    t.write(b"\x1b[=1u\x1b[=2;2u");
    assert_eq!(t.kitty_keyboard_flags().int(), 3);
    t.write(b"\x1b[=1;3u");
    assert_eq!(t.kitty_keyboard_flags().int(), 2);
    for n in 1..=10 {
        t.write(format!("\x1b[>{n}u").as_bytes());
    }
    t.write(b"\x1b[<0u");
    assert_eq!(t.kitty_keyboard_flags().int(), 10);
    t.write(b"\x1b[<7u");
    assert_eq!(t.kitty_keyboard_flags().int(), 3);
    t.write(b"\x1b[?47h");
    assert_eq!(t.kitty_keyboard_flags().int(), 0);
    t.write(b"\x1b[>5u\x1b[?47l");
    assert_eq!(t.kitty_keyboard_flags().int(), 3);
    t.write(b"\x1b[?47h");
    assert_eq!(t.kitty_keyboard_flags().int(), 5);
    t.write(b"\x1b[<65535u");
    assert_eq!(t.kitty_keyboard_flags().int(), 0);
    t.write(b"\x1bc");
    assert_eq!(t.kitty_keyboard_flags().int(), 0);
}

#[test]
fn mode_actions_apply_existing_terminal_owners() {
    use maestro_terminal::{Mode, screen_set::Key};
    let mut t = origin_terminal();
    assert!(t.modes.get(Mode::Origin));
    t.write(b"\x1b[?6l");
    assert_eq!(position(&t), (0, 0));
    t.write(b"\x1b[?69l");
    assert_eq!((t.scrolling_region.left, t.scrolling_region.right), (0, 11));
    for n in [47, 1047, 1049] {
        t.write(format!("\x1b[?{n}h").as_bytes());
        assert_eq!(t.active_screen(), Key::Alternate);
        t.write(b"abc");
        t.write(format!("\x1b[?{n}h").as_bytes());
        if n == 1049 {
            assert_text(&t, "");
        }
        t.write(format!("\x1b[?{n}l\x1b[?{n}l").as_bytes());
        assert_eq!(t.active_screen(), Key::Primary);
    }
    t.write(b"\x1b[2;3H\x1b[?1048h\x1b[H\x1b[?1048l");
    assert_eq!(position(&t), (2, 1));
    t.write(b"\x1b[?3h");
    assert_eq!(t.cols(), 12);
    assert!(!t.modes.get(Mode::Column132));
    t.write(b"\x1b[?40;3h");
    assert_eq!(t.cols(), 132);
    assert!(t.modes.get(Mode::Column132));
    t.write(b"\x1b[?3l");
    assert_eq!(t.cols(), 80);
    t.write(b"\x1b[?7l\x1b[?7s\x1b[?7h\x1b[?7r");
    assert!(!t.modes.get(Mode::Wraparound));
}

#[test]
fn separate_terminals_and_direct_reset_keep_stream_lifetimes() {
    let mut a = Terminal::new(20, 3, 0).unwrap();
    let mut b = Terminal::new(20, 3, 0).unwrap();
    a.write(b"\xc3");
    b.write(b"\xe7\x95");
    a.full_reset();
    a.write(b"\xa9");
    b.write(b"\x8c");
    assert_text(&a, "é");
    assert_text(&b, "界");
    a.write(b"\x1b[2;");
    b.write(b"\x1b]133;");
    a.full_reset();
    a.write(b"3Hx");
    b.write(b"A\x07$");
    assert_eq!(cell(&a, 2, 1).codepoint(), 'x');
    assert_text(&b, "界\n$");
    a.write(b"\x1bcok");
    assert_text(&a, "ok");
    assert_eq!(a.flags.mouse_shift_capture, None);
    a.write(b"\x1b[>1s");
    assert_eq!(a.flags.mouse_shift_capture, Some(true));
    a.write(b"\x1b[>2s");
    assert_eq!(a.flags.mouse_shift_capture, Some(true));
    a.write(b"\x1b[>s");
    assert_eq!(a.flags.mouse_shift_capture, Some(false));
}

#[test]
fn byte_writes_preserve_corrected_erase_and_prompt_owners() {
    use super::support::terminal::{clear_dirty, dirty, row};
    use maestro_terminal::page::{cell::Wide, row::SemanticPrompt};
    for erase in [b"\x1b[?2K".as_slice(), b"\x1b[?2J", b"\x1b[2X"] {
        let mut t = Terminal::new(8, 3, 0).unwrap();
        t.write("\x1bV界\x1bWabc\x1b[2G".as_bytes());
        clear_dirty(&mut t);
        t.write(erase);
        assert_eq!(cell(&t, 0, 0).codepoint(), '界');
        assert_eq!(cell(&t, 0, 0).wide(), Wide::Wide);
        assert_eq!(cell(&t, 1, 0).wide(), Wide::SpacerTail);
        assert!(cell(&t, 2, 0).is_zero());
        assert!(dirty(&t, 0));
    }
    let mut t = Terminal::new(8, 3, 0).unwrap();
    t.write(b"\x1b]133;A\x07$ \x1b]133;B\x07abc\r\x1b[K");
    assert_eq!(row(&t, 0).semantic_prompt(), SemanticPrompt::Prompt);
    t.write(b"\x1bc");
    assert_eq!(row(&t, 0).semantic_prompt(), SemanticPrompt::None);
}

#[test]
fn terminal_bytes_bell_effects_are_drained() {
    let mut t = Terminal::new(10, 3, 0).unwrap();
    t.write(b"\x07hello\x07");
    assert_eq!(t.take_bell_count(), 2);
    assert_eq!(t.take_bell_count(), 0);
    t.write(&vec![7; 65_536]);
    assert_eq!(t.take_bell_count(), u16::MAX);
    assert!(t.take_pty_reply().is_empty());
    assert!(t.take_pwd_changes().is_empty());
    assert!(t.take_clipboard_writes().is_empty());
}

#[test]
fn terminal_bytes_device_replies_are_queued_in_order() {
    let mut t = Terminal::new(80, 24, 0).unwrap();
    t.write(b"\x05\x1b[5n\x1b[c\x1b[>c\x1b[=c\x1b[>q");
    assert_eq!(
        t.take_pty_reply(),
        b"\x1b[0n\x1b[?62;22c\x1b[>1;0;0c\x1bP!|00000000\x1b\\\x1bP>|libghostty\x1b\\"
    );
    assert!(t.take_pty_reply().is_empty());
}

#[test]
fn terminal_bytes_fixed_embedding_replies() {
    let mut t = Terminal::new(80, 24, 0).unwrap();
    t.write(b"\x05");
    assert!(t.take_pty_reply().is_empty());
    t.write(b"\x1b[>0q\x1b[>q");
    assert_eq!(
        t.take_pty_reply(),
        b"\x1bP>|libghostty\x1b\\\x1bP>|libghostty\x1b\\"
    );
}

#[test]
fn terminal_bytes_cursor_report_respects_origin() {
    let mut t = Terminal::new(80, 24, 0).unwrap();
    t.write(b"\x1b[4;8H\x1b[6n");
    assert_eq!(t.take_pty_reply(), b"\x1b[4;8R");
    t.write(b"\x1b[3;20r\x1b[?6h\x1b[4;8H\x1b[6n");
    assert_eq!(t.take_pty_reply(), b"\x1b[4;8R");
}

#[test]
fn terminal_bytes_mode_reports_preserve_namespaces() {
    let mut t = Terminal::new(80, 24, 0).unwrap();
    t.write(b"\x1b[4h\x1b[4$p\x1b[?7l\x1b[?7$p\x1b[1234$p\x1b[?1234$p");
    assert_eq!(
        t.take_pty_reply(),
        b"\x1b[4;1$y\x1b[?7;2$y\x1b[1234;0$y\x1b[?1234;0$y"
    );
}

#[test]
fn terminal_bytes_keyboard_query_tracks_active_flags() {
    let mut t = Terminal::new(80, 24, 0).unwrap();
    t.write(b"\x1b[?u\x1b[>31u\x1b[?u\x1b[<u\x1b[?u");
    assert_eq!(t.take_pty_reply(), b"\x1b[?0u\x1b[?31u\x1b[?0u");
}

#[test]
fn terminal_bytes_size_report_csi_21_t_title() {
    let mut t = Terminal::new(80, 24, 0).unwrap();
    t.set_title(b"stored title");
    t.write(b"\x1b[21t");
    assert_eq!(t.take_pty_reply(), b"\x1b]lstored title\x1b\\");
    t.set_title(b"");
    t.write(b"\x1b[21t");
    assert_eq!(t.take_pty_reply(), b"\x1b]l\x1b\\");
}

#[test]
fn terminal_bytes_color_scheme_queries() {
    use maestro_terminal::device_status::ColorScheme;
    let mut t = Terminal::new(80, 24, 0).unwrap();
    t.write(b"\x1b[?996n");
    assert!(t.take_pty_reply().is_empty());
    t.set_color_scheme(Some(ColorScheme::Dark));
    t.write(b"\x1b[?996n");
    assert_eq!(t.take_pty_reply(), b"\x1b[?997;1n");
    t.set_color_scheme(Some(ColorScheme::Light));
    t.write(b"\x1b[?996n");
    assert_eq!(t.take_pty_reply(), b"\x1b[?997;2n");
    t.set_color_scheme(None);
    t.write(b"\x1b[?996n");
    assert!(t.take_pty_reply().is_empty());
}

#[test]
fn terminal_replies_and_directory_changes_are_captured() {
    let mut t = Terminal::new(8, 3, 100).unwrap();
    t.write(b"\x1b[6n\x1b]7;file:///tmp/maestro\x07");
    assert_eq!(t.take_pty_reply(), b"\x1b[1;1R");
    assert_eq!(t.get_pwd(), Some(b"file:///tmp/maestro".as_slice()));
    assert_eq!(t.take_pwd_changes(), [b"file:///tmp/maestro".to_vec()]);
    assert!(t.take_pwd_changes().is_empty());
}

#[test]
fn osc_title_ingress_caps_bytes_without_capping_direct_setters() {
    let mut t = Terminal::new(10, 3, 0).unwrap();
    for prefix in ["0;", "2;"] {
        t.write(format!("\x1b]{prefix}{}\x07", "é".repeat(513)).as_bytes());
        assert_eq!(t.get_title(), Some("é".repeat(512).as_bytes()));
    }
    t.write(format!("\x1b]2;{}é\x07", "x".repeat(1023)).as_bytes());
    let mut expected = vec![b'x'; 1023];
    expected.push(0xc3);
    assert_eq!(t.get_title(), Some(expected.as_slice()));
    t.write(b"\x1b]2;\x07\x1b[21t");
    assert_eq!(t.get_title(), None);
    assert_eq!(t.take_pty_reply(), b"\x1b]l\x1b\\");
    t.set_title(&vec![b'x'; 1025]);
    assert_eq!(t.get_title().unwrap().len(), 1025);
}

#[test]
fn directory_forms_capture_each_stored_value_in_order() {
    let mut t = Terminal::new(10, 3, 0).unwrap();
    t.write(b"\x1b]7;file:///raw%20path\x07\x1b]1337;currentdir=/tmp\x1b\\");
    t.write(b"\x1b]9;9;/tmp\x07\x1b]7;\x07");
    assert_eq!(t.get_pwd(), None);
    assert_eq!(
        t.take_pwd_changes(),
        [
            b"file:///raw%20path".to_vec(),
            b"/tmp".to_vec(),
            b"/tmp".to_vec(),
            vec![]
        ]
    );
    t.write(b"\x1b]1337;CurrentDir=\x07");
    assert!(t.take_pwd_changes().is_empty());
    t.write(b"\x1b]9;9;\x07");
    assert_eq!(t.take_pwd_changes(), [Vec::<u8>::new()]);
}

#[test]
fn directory_handler_caps_ingress_but_not_direct_setters() {
    use maestro_terminal::{
        stream::{Action, Handler, OscAction},
        stream_terminal,
    };
    let mut t = Terminal::new(10, 3, 0).unwrap();
    for len in [4095, 4096, 4097] {
        stream_terminal::Handler::new(&mut t)
            .vt(Action::Osc(OscAction::ReportPwd(vec![0xff; len])));
        let expected = vec![0xff; len.min(4096)];
        assert_eq!(t.get_pwd(), Some(expected.as_slice()));
        assert_eq!(t.take_pwd_changes(), [expected]);
    }
    t.set_pwd(&vec![b'x'; 4097]);
    assert_eq!(t.get_pwd().unwrap().len(), 4097);
    assert!(t.take_pwd_changes().is_empty());
}

#[test]
fn clipboard_writes_finish_at_either_string_terminator() {
    let mut t = Terminal::new(10, 5, 0).unwrap();
    t.write(b"\x1b]52;c;aGVs");
    assert!(t.take_clipboard_writes().is_empty());
    t.write(b"bG8=\x07");
    assert_eq!(t.take_clipboard_writes(), [b"hello".to_vec()]);
    t.write(b"\x1b]52;c;d29ybGQ=\x1b\\");
    assert_eq!(t.take_clipboard_writes(), [b"world".to_vec()]);
    for byte in b"\x1b]1337;Copy=:Y29weQ==\x1b\\" {
        t.write(&[*byte]);
    }
    assert_eq!(t.take_clipboard_writes(), [b"copy".to_vec()]);
    assert!(t.take_clipboard_writes().is_empty());
}

#[test]
fn clipboard_clear_and_unsupported_targets_are_ignored() {
    let mut t = Terminal::new(10, 5, 0).unwrap();
    for term in ["\x07", "\x1b\\"] {
        for payload in ["c;", "c;?", "s;dGV4dA==", "p;dGV4dA==", ";?"] {
            t.write(format!("\x1b]52;{payload}{term}").as_bytes());
            assert!(t.take_clipboard_writes().is_empty(), "{payload:?}");
        }
        for target in ["c", "x", ""] {
            t.write(format!("\x1b]52;{target};dGV4dA=={term}").as_bytes());
            assert_eq!(t.take_clipboard_writes(), [b"text".to_vec()]);
        }
    }
}

#[test]
fn clipboard_requires_padded_canonical_standard_base64() {
    use maestro_terminal::{
        stream::{Action, Handler, OscAction},
        stream_terminal,
    };
    let mut t = Terminal::new(10, 5, 0).unwrap();
    for payload in [
        "aA", "aA=", "aA===", "aB==", "aGV=", "a A==", "_w==", "-w==", "!!!!", "=aA=", "aA==YQ==",
    ] {
        for ending in ["\x07", "\x1b\\"] {
            t.write(format!("\x1b]52;c;{payload}{ending}").as_bytes());
            assert!(t.take_clipboard_writes().is_empty(), "{payload:?}");
        }
    }
    for whitespace in [b'\t', b'\n', b'\r', 12] {
        let mut data = b"aA==".to_vec();
        data.push(whitespace);
        stream_terminal::Handler::new(&mut t).vt(Action::Osc(OscAction::ClipboardContents {
            kind: b'c',
            data,
        }));
        assert!(t.take_clipboard_writes().is_empty());
        t.write(format!("\x1b]52;c;aA=={}\x07", char::from(whitespace)).as_bytes());
        assert_eq!(t.take_clipboard_writes(), [b"h".to_vec()]);
    }
    t.write(b"\x1b]52;c;aA==\x07\x1b]52;c;aGU=\x07\x1b]52;c;aGVs\x07");
    t.write(b"\x1b]52;c;/wA=\x07");
    assert_eq!(
        t.take_clipboard_writes(),
        [b"h".to_vec(), b"he".to_vec(), b"hel".to_vec(), vec![255, 0]]
    );
}

#[test]
fn clipboard_size_limit_is_on_decoded_bytes() {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    let mut t = Terminal::new(10, 5, 0).unwrap();
    let at_limit = vec![b'x'; 192 * 1024];
    t.write(format!("\x1b]52;c;{}\x07", STANDARD.encode(&at_limit)).as_bytes());
    assert_eq!(t.take_clipboard_writes(), [at_limit]);
    let over_limit = vec![b'x'; 192 * 1024 + 1];
    t.write(format!("\x1b]52;c;{}\x1b\\", STANDARD.encode(over_limit)).as_bytes());
    assert!(t.take_clipboard_writes().is_empty());
    t.write(b"\x1b]52;c;b2s=\x07");
    assert_eq!(t.take_clipboard_writes(), [b"ok".to_vec()]);
}
