//! Terminal text reads retain physical rows and Unicode cell contents.

use maestro_terminal::{
    Terminal,
    page::cell::Wide,
    terminal::{TextRange, TextShape},
};

fn range(start: (u16, u32), end: (u16, u32), shape: TextShape) -> TextRange {
    TextRange { start, end, shape }
}

#[test]
fn text_rows_retain_wrap_flags_and_graphemes() {
    let mut terminal = Terminal::new(5, 3, 100_000).unwrap();
    terminal.write("abcdef\r\n界e\u{301}".as_bytes());
    let rows = terminal.screen_text_rows().unwrap();
    assert_eq!(rows.len(), 3);
    assert!(rows[0].soft_wrapped);
    assert!(!rows[0].wrap_continuation);
    assert!(!rows[1].soft_wrapped);
    assert!(rows[1].wrap_continuation);
    assert!(!rows[2].wrap_continuation);
    assert_eq!(rows[2].cells[0].wide, Wide::Wide);
    assert_eq!(rows[2].cells[0].graphemes, vec!['界']);
    assert_eq!(rows[2].cells[1].wide, Wide::SpacerTail);
    assert!(rows[2].cells[1].graphemes.is_empty());
    assert_eq!(rows[2].cells[2].graphemes, vec!['e', '\u{301}']);
    assert_eq!(
        terminal.screen_cell(2, 2),
        Some((Wide::Narrow, vec!['e', '\u{301}']))
    );
    assert_eq!(terminal.screen_cell(5, 2), None);
    assert_eq!(terminal.screen_cell(0, 3), None);
}

#[test]
fn viewport_links_return_the_authored_uri() {
    let mut terminal = Terminal::new(20, 3, 100_000).unwrap();
    terminal.write(b"\x1b]8;;https://example.com\x1b\\Link\x1b]8;;\x1b\\");
    assert_eq!(
        terminal.viewport_hyperlink_uri(0, 0).unwrap().as_deref(),
        Some("https://example.com")
    );
    assert_eq!(terminal.viewport_hyperlink_uri(4, 0), Some(None));
    assert_eq!(terminal.viewport_hyperlink_uri(20, 0), None);
    assert_eq!(terminal.viewport_hyperlink_uri(0, 3), None);
}

#[test]
fn screen_text_reads_unwrap_trim_and_order_the_selection() {
    let mut terminal = Terminal::new(5, 3, 100_000).unwrap();
    terminal.write(b"1ABCD2EFGH3IJKL");
    assert_eq!(
        terminal
            .read_text_screen(range((0, 1), (2, 2), TextShape::Linear))
            .unwrap(),
        "2EFGH3IJ"
    );
    assert_eq!(
        terminal
            .read_text_screen(range((2, 2), (0, 1), TextShape::Linear))
            .unwrap(),
        "2EFGH3IJ"
    );
    assert_eq!(
        terminal
            .read_text_screen(range((1, 0), (3, 2), TextShape::Rectangle))
            .unwrap(),
        "ABCEFGIJK"
    );
    assert!(
        terminal
            .read_text_screen(range((5, 0), (0, 1), TextShape::Linear))
            .is_none()
    );
    assert!(
        terminal
            .read_text_screen(range((0, 0), (0, 3), TextShape::Linear))
            .is_none()
    );
}

#[test]
fn styled_screen_and_viewport_reads_keep_their_own_origins() {
    use maestro_terminal::terminal::ScrollViewport;
    let mut terminal = Terminal::new(5, 2, 100_000).unwrap();
    terminal.write(b"old\r\n\x1b[1mABCDE12345Z\x1b[0m");
    let selection = range((0, 0), (4, 1), TextShape::Linear);
    assert_eq!(terminal.read_text_screen(selection).unwrap(), "old\nABCDE");
    assert_eq!(
        terminal.read_ansi_screen(selection, false).unwrap(),
        "old\r\n\x1b[0m\x1b[1mABCDE\x1b[0m"
    );
    assert_eq!(
        terminal.read_ansi_viewport(selection).unwrap(),
        "\x1b[0m\x1b[1m12345\x1b[0m\r\n\x1b[0m\x1b[1mZ\x1b[0m"
    );
    terminal.scroll_viewport(ScrollViewport::Top);
    assert_eq!(
        terminal.read_ansi_viewport(selection),
        terminal.read_ansi_screen(selection, false)
    );
    let wrapped = range((0, 1), (0, 3), TextShape::Linear);
    assert_eq!(
        terminal.read_ansi_screen(wrapped, true).unwrap(),
        "\x1b[0m\x1b[1mABCDE12345Z\x1b[0m"
    );
}

#[test]
fn keyboard_replay_excludes_cells_and_restores_both_keyboard_modes() {
    let mut terminal = Terminal::new(20, 3, 100_000).unwrap();
    terminal.write(b"must not appear");
    assert_eq!(terminal.kitty_keyboard_state_ansi().unwrap(), "");
    terminal.write(b"\x1b[>4;2m\x1b[=5;1u");
    let replay = terminal.kitty_keyboard_state_ansi().unwrap();
    assert_eq!(replay, "\x1b[=5u\x1b[>4;2m");
    let mut restored = Terminal::new(20, 3, 100_000).unwrap();
    restored.write(replay.as_bytes());
    assert_eq!(restored.kitty_keyboard_flags().int(), 5);
    assert!(restored.modify_other_keys_enabled());
    terminal.write(b"\x1b[?1049h");
    assert_eq!(terminal.kitty_keyboard_state_ansi().unwrap(), "\x1b[>4;2m");
}

#[test]
fn tracked_row_reports_the_previous_viewport_anchor_after_scrolling() {
    let mut terminal = Terminal::new(5, 2, 100_000).unwrap();
    terminal.write(b"A\r\nB");
    let count = terminal.screens.active().pages.count_tracked_pins();
    assert_eq!(terminal.track_row(1), None);
    assert_eq!(
        terminal.screens.active().pages.count_tracked_pins(),
        count + 1
    );
    terminal.write(b"\r\nC");
    assert_eq!(terminal.track_row(1), Some(1));
    assert_eq!(
        terminal.screens.active().pages.count_tracked_pins(),
        count + 1
    );
    assert_eq!(terminal.track_row(u32::MAX), Some(2));
    assert_eq!(terminal.screens.active().pages.count_tracked_pins(), count);
    assert_eq!(terminal.track_row(0), None);
    terminal.full_reset();
    assert_eq!(terminal.track_row(0), None);
}

#[test]
fn row_ranges_are_half_open_clamped_and_include_history() {
    let mut terminal = Terminal::new(5, 2, 100_000).unwrap();
    terminal.write(b"one\r\ntwo\r\nthree");
    let all = terminal.screen_text_rows().unwrap();
    assert_eq!(all.len(), 3);
    assert_eq!(terminal.screen_text_rows_range(1, 2).unwrap(), all[1..2]);
    assert_eq!(
        terminal.screen_text_rows_range(2, usize::MAX).unwrap(),
        all[2..]
    );
    assert!(terminal.screen_text_rows_range(2, 1).unwrap().is_empty());
    assert!(
        terminal
            .screen_text_rows_range(usize::MAX, usize::MAX)
            .unwrap()
            .is_empty()
    );
    assert_eq!(terminal.screen_cell(0, 0), Some((Wide::Narrow, vec!['o'])));
    assert_eq!(terminal.screen_cell(4, 0), Some((Wide::Narrow, vec![])));
}

fn viewport_text(terminal: &Terminal, range: TextRange) -> String {
    use maestro_terminal::{
        Coordinate, Point,
        formatter::{Content, Options, TerminalFormatter},
        selection::Selection,
    };
    let pages = &terminal.screens.active().pages;
    let mut formatter = TerminalFormatter::init(
        terminal,
        Options {
            unwrap: true,
            ..Options::default()
        },
    );
    formatter.content = Content::Selection(Some(Selection::init(
        pages
            .pin(Point::Viewport(Coordinate {
                x: range.start.0,
                y: range.start.1,
            }))
            .unwrap(),
        pages
            .pin(Point::Viewport(Coordinate {
                x: range.end.0,
                y: range.end.1,
            }))
            .unwrap(),
        range.shape == TextShape::Rectangle,
    )));
    let mut text = String::new();
    formatter.format(&mut text).unwrap();
    text
}

#[test]
fn soft_wrapped_viewport_selection_joins_lines() {
    let mut terminal = Terminal::new(5, 3, 100_000).unwrap();
    terminal.write(b"1ABCD2EFGH3IJKL");
    assert_eq!(
        viewport_text(&terminal, range((0, 1), (2, 2), TextShape::Linear)),
        "2EFGH3IJ"
    );
}

#[test]
fn viewport_selection_preserves_wide_characters() {
    let mut terminal = Terminal::new(5, 3, 100_000).unwrap();
    terminal.write("1A⚡".as_bytes());
    for (start, end, expected) in [(0, 3, "1A⚡"), (0, 2, "1A⚡"), (3, 3, "⚡")] {
        assert_eq!(
            viewport_text(&terminal, range((start, 0), (end, 0), TextShape::Linear)),
            expected
        );
    }
}

#[test]
fn tracked_rows_keep_screen_ownership_and_reject_removed_lifetimes() {
    let mut terminal = Terminal::new(5, 3, 100_000).unwrap();
    assert_eq!(terminal.track_row(2), None);
    let count = terminal.screens.active().pages.count_tracked_pins();
    terminal.write(b"\x1b[?1049h");
    assert_eq!(terminal.track_row(1), Some(2));
    assert_eq!(
        terminal
            .screens
            .get(maestro_terminal::screen_set::Key::Primary)
            .unwrap()
            .pages
            .count_tracked_pins(),
        count - 1
    );
    terminal.write(b"\x1b[?1049l");
    assert_eq!(terminal.track_row(0), Some(1));
    terminal.write(b"\x1b[?1049h");
    assert_eq!(terminal.track_row(0), Some(0));
    terminal.full_reset();
    terminal.write(b"\x1b[?1049h");
    assert_eq!(terminal.track_row(0), None);
    assert_eq!(terminal.track_row(1), Some(0));
}

#[test]
fn screen_reads_keep_wide_characters_graphemes_and_trimmed_lines() {
    let mut terminal = Terminal::new(12, 3, 100_000).unwrap();
    terminal.write("界e\u{301}  \r\nend  ".as_bytes());
    assert_eq!(
        terminal
            .read_text_screen(range((1, 0), (11, 1), TextShape::Linear))
            .unwrap(),
        "界e\u{301}\nend"
    );
    assert_eq!(
        terminal
            .read_ansi_screen(range((0, 0), (11, 1), TextShape::Linear), false)
            .unwrap(),
        "界e\u{301}  \r\nend  "
    );
    assert_eq!(terminal.screen_cell(1, 0), Some((Wide::SpacerTail, vec![])));
    assert!(
        terminal
            .read_ansi_viewport(range((0, 0), (0, 3), TextShape::Linear))
            .is_none()
    );
    assert!(
        terminal
            .read_ansi_screen(range((0, 0), (12, 0), TextShape::Linear), false)
            .is_none()
    );
}

#[test]
fn resizing_deep_history_preserves_text_and_links() {
    use maestro_terminal::terminal::ScrollViewport;
    use std::fmt::Write as _;
    let mut terminal = Terminal::new(20, 5, 100_000_000).unwrap();
    let mut input = String::from("\x1b]8;;https://example.com\x1b\\FIRST 🇧🇷\x1b]8;;\x1b\\\r\n");
    for line in 0..70_000 {
        writeln!(input, "{line:05} 👨‍👩‍👧").unwrap();
    }
    terminal.write(input.as_bytes());
    assert!(terminal.total_rows() - usize::from(terminal.rows()) > usize::from(u16::MAX));
    for cols in [20, 10] {
        terminal.resize(cols, 5, 8, 16).unwrap();
        terminal.scroll_viewport(ScrollViewport::Top);
        assert_eq!(terminal.scrollbar().offset, 0);
        assert_eq!(terminal.scrollbar().len, 5);
        assert_eq!(
            viewport_text(&terminal, range((0, 0), (cols - 1, 0), TextShape::Linear)),
            "FIRST 🇧🇷"
        );
        assert_eq!(
            terminal.viewport_hyperlink_uri(0, 0).unwrap().as_deref(),
            Some("https://example.com")
        );
    }
}

fn resized_history(old_cols: u16) -> Terminal {
    let mut terminal = Terminal::new(old_cols, 3, 10_000_000).unwrap();
    let line: String = (0..old_cols)
        .map(|x| char::from(b'A' + u8::try_from(x % 26).unwrap()))
        .collect();
    for _ in 0..100 {
        terminal.write(line.as_bytes());
        terminal.write(b"\r\n");
    }
    terminal.resize(80, 3, 8, 16).unwrap();
    terminal
}

fn cell_text(cells: &[maestro_terminal::terminal::ScreenTextCell]) -> String {
    cells
        .iter()
        .flat_map(|cell| cell.graphemes.iter().copied())
        .collect()
}

#[test]
fn widening_history_pads_rows_to_current_width() {
    let terminal = resized_history(40);
    let rows = terminal.screen_text_rows_range(0, 2).unwrap();
    assert_eq!(rows.len(), 2);
    for row in rows {
        assert_eq!(row.cells.len(), 80);
        assert_eq!(
            cell_text(&row.cells),
            "ABCDEFGHIJKLMNOPQRSTUVWXYZABCDEFGHIJKLMN"
        );
        assert!(
            row.cells[40..]
                .iter()
                .all(|cell| cell.graphemes.is_empty() && cell.wide == Wide::Narrow)
        );
        assert!(!row.soft_wrapped);
        assert!(!row.wrap_continuation);
    }
}

#[test]
fn narrowing_history_reflows_rows_at_current_width() {
    let terminal = resized_history(120);
    let rows = terminal.screen_text_rows_range(0, 2).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].cells.len(), 80);
    assert_eq!(rows[1].cells.len(), 80);
    assert_eq!(
        cell_text(&rows[0].cells),
        "ABCDEFGHIJKLMNOPQRSTUVWXYZABCDEFGHIJKLMNOPQRSTUVWXYZABCDEFGHIJKLMNOPQRSTUVWXYZAB"
    );
    assert_eq!(
        cell_text(&rows[1].cells),
        "CDEFGHIJKLMNOPQRSTUVWXYZABCDEFGHIJKLMNOP"
    );
    assert!(rows[0].soft_wrapped);
    assert!(!rows[0].wrap_continuation);
    assert!(!rows[1].soft_wrapped);
    assert!(rows[1].wrap_continuation);
    assert!(
        rows[1].cells[40..]
            .iter()
            .all(|cell| cell.graphemes.is_empty())
    );
}

#[test]
fn unchanged_history_width_retains_complete_physical_rows() {
    let terminal = resized_history(80);
    let rows = terminal.screen_text_rows_range(0, 2).unwrap();
    assert_eq!(rows.len(), 2);
    for row in rows {
        assert_eq!(row.cells.len(), 80);
        assert_eq!(
            cell_text(&row.cells),
            "ABCDEFGHIJKLMNOPQRSTUVWXYZABCDEFGHIJKLMNOPQRSTUVWXYZABCDEFGHIJKLMNOPQRSTUVWXYZAB"
        );
        assert!(!row.soft_wrapped);
        assert!(!row.wrap_continuation);
    }
}

#[test]
fn cell_read_content_retains_width_style_and_link_state() {
    let mut terminal = Terminal::new(8, 3, 100_000).unwrap();
    terminal.write(b"\x1b[31mA\x1b[0m");
    let mut state = maestro_terminal::render::RenderState::default();
    state.update(&mut terminal);
    let row = state.rows().next().unwrap();
    let cell = row.cell(0).unwrap();
    assert_eq!(cell.raw().wide(), Wide::Narrow);
    assert!(cell.has_styling());
    assert_eq!(
        cell.style().fg_color,
        maestro_terminal::style::Color::Palette(1)
    );
    assert!(!cell.raw().hyperlink());
    assert_eq!(cell.graphemes_utf8(), "A");
}
