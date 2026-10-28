//! Byte input through terminal reads, viewport selection and reset.

use super::support::{
    LARGE_PAGES,
    terminal::{assert_text, cell},
};
use maestro_terminal::{
    Coordinate, Mode, Point, Terminal, screen_set::Key, terminal::ScrollViewport,
};

fn viewport(t: &Terminal) -> String {
    let pages = &t.screens.active().pages;
    let mut lines = Vec::new();
    for y in 0..u32::from(t.rows()) {
        let pin = pages.pin(Point::Viewport(Coordinate { x: 0, y })).unwrap();
        let text: String = pages
            .page(pin.node)
            .unwrap()
            .get_cells(pin.y)
            .iter()
            .map(|c| {
                if c.codepoint() == '\0' {
                    ' '
                } else {
                    c.codepoint()
                }
            })
            .collect();
        lines.push(text.trim_end().to_owned());
    }
    while lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    lines.join("\n")
}

#[test]
fn terminal_input_scroll_viewport() {
    let mut t = Terminal::new(5, 2, 10_000).unwrap();
    t.write(b"hello\x1bD\x1bD\x1bD");
    assert_eq!(viewport(&t), "");
    assert!(t.scrollbar().offset > 0);
    t.scroll_viewport(ScrollViewport::Top);
    assert_eq!(t.scrollbar().offset, 0);
    assert_eq!(viewport(&t), "hello");
    t.scroll_viewport(ScrollViewport::Bottom);
    assert_eq!(viewport(&t), "");
    t.scroll_viewport(ScrollViewport::Delta(-3));
    assert_eq!(viewport(&t), "hello");
}

#[test]
fn terminal_input_scroll_viewport_row() {
    let mut t = Terminal::new(5, 2, 10_000).unwrap();
    t.write(b"1\r\n2\r\n3\r\n4");
    for (row, text, offset) in [(0, "1\n2", 0), (1, "2\n3", 1), (9999, "3\n4", 2)] {
        t.scroll_viewport(ScrollViewport::Row(row));
        assert_eq!(viewport(&t), text);
        let bar = t.scrollbar();
        assert_eq!((bar.total, bar.offset, bar.len), (4, offset, 2));
    }
}

#[test]
fn terminal_input_scroll_viewport_row_alt_screen() {
    let mut t = Terminal::new(5, 2, 10_000).unwrap();
    t.write(b"\x1b[?1049h");
    assert_eq!(t.active_screen(), Key::Alternate);
    for row in [0, 9999] {
        t.scroll_viewport(ScrollViewport::Row(row));
        let bar = t.scrollbar();
        assert_eq!((bar.total, bar.offset, bar.len), (2, 0, 2));
    }
}

#[test]
fn terminal_input_reset() {
    let mut t = Terminal::new(80, 24, 10_000).unwrap();
    t.write(b"Hello");
    t.full_reset();
    assert_text(&t, "");
}

#[test]
fn terminal_input_resize_shrinks_both_axes_with_cursor_at_bottom() {
    let mut t = Terminal::new(80, 24, 0).unwrap();
    t.write(b"\x1b[24;1H");
    assert_eq!(t.screens.active().cursor.y, 23);
    t.resize(79, 23, 8, 16).unwrap();
    assert_eq!((t.cols(), t.rows()), (79, 23));
    assert!(t.screens.active().cursor.y < 23);
}

#[test]
fn terminal_input_vt_write() {
    let mut t = Terminal::new(80, 24, 10_000).unwrap();
    t.write(b"Hello");
    assert_text(&t, "Hello");
}

#[test]
fn terminal_input_vt_write_split_escape_sequence() {
    let mut t = Terminal::new(80, 24, 10_000).unwrap();
    t.write(b"Hello \x1b");
    t.write(b"[1mBold\x1b[0m");
    assert_text(&t, "Hello Bold");
    let pages = &t.screens.active().pages;
    let pin = pages.pin(Point::Active(Coordinate { x: 6, y: 0 })).unwrap();
    assert!(pin.style(pages, cell(&t, 6, 0)).unwrap().flags.bold());
    assert!(!t.screens.active().cursor.style.flags.bold());
}

#[test]
fn terminal_input_vt_write_split_combining_mark_after_base_at_right_edge() {
    let mut t = Terminal::new(2, 2, 0).unwrap();
    t.write("xå".as_bytes());
    assert!(t.screens.active().cursor.pending_wrap);
    t.write(b"\xcc\xb2");
    assert_text(&t, "xå\u{332}");
    assert_eq!(t.screens.active().cursor.y, 0);
}

#[test]
fn terminal_input_get_cursor_position() {
    let mut t = Terminal::new(80, 24, 0).unwrap();
    t.write(b"Hello");
    let cursor = &t.screens.active().cursor;
    assert_eq!((cursor.x, cursor.y), (5, 0));
}

#[test]
fn terminal_input_get_kitty_keyboard_flags() {
    let mut t = Terminal::new(80, 24, 0).unwrap();
    assert_eq!(t.kitty_keyboard_flags().int(), 0);
    t.write(b"\x1b[>3u");
    assert_eq!(t.kitty_keyboard_flags().int(), 3);
}

#[test]
fn extended_key_queries_follow_mode_two() {
    let mut t = Terminal::new(80, 24, 0).unwrap();
    assert!(!t.modify_other_keys_enabled());
    t.write(b"\x1b[>4;2m");
    assert!(t.modify_other_keys_enabled());
    t.write(b"\x1b[>4;0m");
    assert!(!t.modify_other_keys_enabled());
}

#[test]
fn terminal_input_get_mouse_tracking() {
    let mut t = Terminal::new(80, 24, 0).unwrap();
    assert!(!t.mouse_tracking_enabled());
    for mode in [9, 1000, 1002, 1003] {
        t.write(format!("\x1b[?{mode}h").as_bytes());
        assert!(t.mouse_tracking_enabled());
        t.write(format!("\x1b[?{mode}l").as_bytes());
        assert!(!t.mouse_tracking_enabled());
    }
}

#[test]
fn terminal_input_grid_ref() {
    let mut t = Terminal::new(80, 24, 0).unwrap();
    t.write(b"Hello");
    assert_eq!(cell(&t, 0, 0).codepoint(), 'H');
}

fn numbered_lines(t: &mut Terminal, count: usize) {
    for i in 0..count {
        t.write(format!("{i}\r\n").as_bytes());
    }
}

#[test]
fn zero_history_budget_disables_scrollback() {
    let mut t = Terminal::new(80, 3, 0).unwrap();
    numbered_lines(&mut t, 3000);
    assert_eq!(t.total_rows(), 3);
    assert_eq!(viewport(&t), "2998\n2999");
    t.scroll_viewport(ScrollViewport::Top);
    assert_eq!(viewport(&t), "2998\n2999");
}

#[test]
fn larger_byte_budgets_retain_more_history() {
    let mut retained = Vec::new();
    // 16 KiB host pages hold more rows each, so pruning whole pages at the smaller budget
    // leaves a different history.
    let smaller = if LARGE_PAGES { 657 } else { 669 };
    for (budget, minimum) in [(1_000_000, smaller), (10_000_000, 1248)] {
        let mut t = Terminal::new(80, 3, budget).unwrap();
        let line = format!("{}\r\n", "x".repeat(70));
        for _ in 0..1250 {
            t.write(line.as_bytes());
        }
        let history = t.total_rows() - 3;
        assert!(history >= minimum, "budget {budget}: {history}");
        assert_eq!(viewport(&t), format!("{0}\n{0}", "x".repeat(70)));
        let pages = &t.screens.active().pages;
        assert!(pages.charged_bytes() <= pages.max_size());
        eprintln!(
            "budget={budget} history={history} charged={} limit={}",
            pages.charged_bytes(),
            pages.max_size()
        );
        retained.push(history);
    }
    assert!(retained[1] > retained[0]);
}

#[test]
fn negative_scroll_saturates_at_the_oldest_row() {
    let mut t = Terminal::new(80, 3, 1_000_000).unwrap();
    numbered_lines(&mut t, 1000);
    let bar = t.scrollbar();
    assert!(bar.offset > 0 && bar.total > bar.len);
    let (len, newest) = (bar.len, viewport(&t));
    t.scroll_viewport(ScrollViewport::Top);
    let oldest = viewport(&t);
    assert_ne!(oldest, newest);
    t.scroll_viewport(ScrollViewport::Bottom);
    t.scroll_viewport(ScrollViewport::Delta(-10_000));
    let bar = t.scrollbar();
    assert_eq!((bar.offset, bar.len), (0, len));
    assert_eq!(viewport(&t), oldest);
}

#[test]
fn absolute_scroll_positions_round_trip_and_saturate() {
    let mut t = Terminal::new(80, 3, 1_000_000).unwrap();
    numbered_lines(&mut t, 1000);
    let bar = t.scrollbar();
    let max = bar.total - bar.len;
    assert!(max > 1, "retained history: {max}");
    for row in [0, max / 2, max, usize::MAX] {
        t.scroll_viewport(ScrollViewport::Row(row));
        let current = t.scrollbar();
        assert_eq!((current.offset, current.len), (row.min(max), bar.len));
    }
}

#[test]
fn reset_keeps_default_grapheme_clustering() {
    let mut t = Terminal::new(2, 2, 0).unwrap();
    assert!(t.modes.get(Mode::GraphemeCluster));
    t.write(b"\x1bc");
    assert!(t.modes.get(Mode::GraphemeCluster));
    t.write("xå".as_bytes());
    t.write(b"\xcc\xb2");
    assert_text(&t, "xå\u{332}");
}
