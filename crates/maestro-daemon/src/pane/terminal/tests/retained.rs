//! Retained Unicode search and word motion.
use super::super::{retained::*, *};
use maestro_terminal::{
    Terminal,
    page::cell::Wide,
    screen_set::Key,
    terminal::{ScreenTextCell, ScreenTextRow},
};
fn text_cell(text: &str) -> ScreenTextCell {
    ScreenTextCell {
        wide: Wide::Narrow,
        graphemes: text.chars().collect(),
    }
}

fn wide_text_cells(text: &str) -> [ScreenTextCell; 2] {
    [
        ScreenTextCell {
            wide: Wide::Wide,
            graphemes: text.chars().collect(),
        },
        ScreenTextCell {
            wide: Wide::SpacerTail,
            graphemes: Vec::new(),
        },
    ]
}

fn text_row(cells: impl IntoIterator<Item = ScreenTextCell>, soft_wrapped: bool) -> ScreenTextRow {
    ScreenTextRow {
        cells: cells.into_iter().collect(),
        soft_wrapped,
        wrap_continuation: false,
    }
}

fn search_primary(
    buffer: &RetainedTextBuffer,
    query: &str,
    case_sensitive: bool,
) -> Vec<TerminalTextMatch> {
    buffer.search(query, case_sensitive, Key::Primary)
}
#[test]
fn pane_retained_text_search_crosses_soft_wraps_but_not_hard_lines() {
    let buffer = RetainedTextBuffer::new(
        5,
        vec![
            text_row("abcde".chars().map(|ch| text_cell(&ch.to_string())), true),
            text_row("fgh  ".chars().map(|ch| text_cell(&ch.to_string())), false),
            text_row("abc  ".chars().map(|ch| text_cell(&ch.to_string())), false),
        ],
    );

    let matches = search_primary(&buffer, "def", true);
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].start, TerminalTextPoint { row: 0, col: 3 });
    assert_eq!(matches[0].end, TerminalTextPoint { row: 1, col: 0 });
    assert!(search_primary(&buffer, "hab", true).is_empty());
}

#[test]
fn pane_retained_text_search_maps_wide_and_combining_graphemes_to_cells() {
    let mut cells = vec![text_cell("A")];
    cells.extend(wide_text_cells("界"));
    cells.push(text_cell("e\u{301}"));
    cells.push(text_cell("Z"));
    let buffer = RetainedTextBuffer::new(5, vec![text_row(cells, false)]);

    let matches = search_primary(&buffer, "界e\u{301}", true);
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].start, TerminalTextPoint { row: 0, col: 1 });
    assert_eq!(matches[0].end, TerminalTextPoint { row: 0, col: 3 });
    assert!(search_primary(&buffer, "\u{301}", true).is_empty());
}

#[test]
fn pane_retained_text_search_skips_wide_spacer_heads_at_soft_wraps() {
    let mut first = "abcd"
        .chars()
        .map(|ch| text_cell(&ch.to_string()))
        .collect::<Vec<_>>();
    first.push(ScreenTextCell {
        wide: Wide::SpacerHead,
        graphemes: Vec::new(),
    });
    let mut second = wide_text_cells("界").to_vec();
    second.extend("xyz".chars().map(|ch| text_cell(&ch.to_string())));
    let buffer = RetainedTextBuffer::new(5, vec![text_row(first, true), text_row(second, false)]);

    let matches = search_primary(&buffer, "d界", true);
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].start, TerminalTextPoint { row: 0, col: 3 });
    assert_eq!(matches[0].end, TerminalTextPoint { row: 1, col: 1 });
}

#[test]
fn pane_retained_text_word_motion_does_not_split_at_a_wide_spacer_head() {
    let mut first = "abcd"
        .chars()
        .map(|ch| text_cell(&ch.to_string()))
        .collect::<Vec<_>>();
    first.push(ScreenTextCell {
        wide: Wide::SpacerHead,
        graphemes: Vec::new(),
    });
    let mut second = wide_text_cells("界").to_vec();
    second.extend("xyz".chars().map(|ch| text_cell(&ch.to_string())));
    let buffer = RetainedTextBuffer::new(5, vec![text_row(first, true), text_row(second, false)]);

    assert_eq!(
        buffer.word_motion(0, 0, TerminalWordMotion::NextStart),
        None
    );
    assert_eq!(
        buffer.word_motion(0, 0, TerminalWordMotion::NextEnd),
        Some(TerminalTextPoint { row: 1, col: 4 })
    );
}

#[test]
fn pane_retained_text_search_is_literal_and_unicode_case_aware() {
    let buffer = RetainedTextBuffer::new(
        12,
        vec![text_row(
            "CAFÉ a.b    ".chars().map(|ch| text_cell(&ch.to_string())),
            false,
        )],
    );

    assert_eq!(search_primary(&buffer, "café", false).len(), 1);
    assert!(search_primary(&buffer, "café", true).is_empty());
    assert_eq!(search_primary(&buffer, "a.b", true).len(), 1);
    assert!(search_primary(&buffer, "a?b", true).is_empty());
}

#[test]
fn pane_retained_text_word_motions_use_tmux_separators_across_rows() {
    let buffer = RetainedTextBuffer::new(
        6,
        vec![
            text_row("a_b.c ".chars().map(|ch| text_cell(&ch.to_string())), false),
            text_row("—d    ".chars().map(|ch| text_cell(&ch.to_string())), false),
        ],
    );

    assert_eq!(
        buffer.word_motion(0, 0, TerminalWordMotion::NextStart),
        Some(TerminalTextPoint { row: 0, col: 3 })
    );
    assert_eq!(
        buffer.word_motion(0, 3, TerminalWordMotion::NextStart),
        Some(TerminalTextPoint { row: 0, col: 4 })
    );
    assert_eq!(
        buffer.word_motion(0, 4, TerminalWordMotion::NextStart),
        Some(TerminalTextPoint { row: 1, col: 0 })
    );
    assert_eq!(
        buffer.word_motion(1, 1, TerminalWordMotion::PreviousStart),
        Some(TerminalTextPoint { row: 1, col: 0 })
    );
}

#[test]
fn pane_retained_text_big_word_motions_treat_only_whitespace_as_separators() {
    let buffer = RetainedTextBuffer::new(
        20,
        vec![text_row(
            "foo.bar baz qux/quux"
                .chars()
                .map(|ch| text_cell(&ch.to_string())),
            false,
        )],
    );

    // `W` skips punctuation-separated segments and lands on the next
    // whitespace-delimited run.
    assert_eq!(
        buffer.word_motion(0, 0, TerminalWordMotion::NextBigStart),
        Some(TerminalTextPoint { row: 0, col: 8 })
    );
    assert_eq!(
        buffer.word_motion(0, 8, TerminalWordMotion::NextBigStart),
        Some(TerminalTextPoint { row: 0, col: 12 })
    );
    // `E` lands on the last character of the current/next run.
    assert_eq!(
        buffer.word_motion(0, 0, TerminalWordMotion::NextBigEnd),
        Some(TerminalTextPoint { row: 0, col: 6 })
    );
    assert_eq!(
        buffer.word_motion(0, 6, TerminalWordMotion::NextBigEnd),
        Some(TerminalTextPoint { row: 0, col: 10 })
    );
    assert_eq!(
        buffer.word_motion(0, 12, TerminalWordMotion::NextBigEnd),
        Some(TerminalTextPoint { row: 0, col: 19 })
    );
    // `B` returns to the beginning of the previous run.
    assert_eq!(
        buffer.word_motion(0, 19, TerminalWordMotion::PreviousBigStart),
        Some(TerminalTextPoint { row: 0, col: 12 })
    );
    assert_eq!(
        buffer.word_motion(0, 12, TerminalWordMotion::PreviousBigStart),
        Some(TerminalTextPoint { row: 0, col: 8 })
    );
    assert_eq!(
        buffer.word_motion(0, 8, TerminalWordMotion::PreviousBigStart),
        Some(TerminalTextPoint { row: 0, col: 0 })
    );

    // Lowercase motions keep their punctuation-aware behavior.
    assert_eq!(
        buffer.word_motion(0, 0, TerminalWordMotion::NextStart),
        Some(TerminalTextPoint { row: 0, col: 3 })
    );
    assert_eq!(
        buffer.word_motion(0, 3, TerminalWordMotion::NextStart),
        Some(TerminalTextPoint { row: 0, col: 4 })
    );
    assert_eq!(
        buffer.word_motion(0, 4, TerminalWordMotion::PreviousStart),
        Some(TerminalTextPoint { row: 0, col: 3 })
    );
}

#[test]
fn pane_retained_text_big_word_motions_cross_rows_and_blank_lines() {
    let buffer = RetainedTextBuffer::new(
        6,
        vec![
            text_row("a.b-c ".chars().map(|ch| text_cell(&ch.to_string())), false),
            text_row("      ".chars().map(|ch| text_cell(&ch.to_string())), false),
            text_row("d_e   ".chars().map(|ch| text_cell(&ch.to_string())), false),
        ],
    );

    assert_eq!(
        buffer.word_motion(0, 0, TerminalWordMotion::NextBigStart),
        Some(TerminalTextPoint { row: 2, col: 0 })
    );
    assert_eq!(
        buffer.word_motion(2, 0, TerminalWordMotion::PreviousBigStart),
        Some(TerminalTextPoint { row: 0, col: 0 })
    );
    assert_eq!(
        buffer.word_motion(0, 0, TerminalWordMotion::NextBigEnd),
        Some(TerminalTextPoint { row: 0, col: 4 })
    );
    assert_eq!(
        buffer.word_motion(0, 4, TerminalWordMotion::NextBigEnd),
        Some(TerminalTextPoint { row: 2, col: 2 })
    );
}

#[test]
fn pane_live_terminal_match_validation_rejects_overwritten_text() {
    let mut terminal = Terminal::new(20, 3, 100).unwrap();
    terminal.write(b"alpha needle");
    let pane = PaneTerminal::new(terminal);

    let text_match = pane.search_text_matches("needle", true)[0];
    assert!(pane.text_match_is_current(text_match));
    pane.core
        .lock()
        .unwrap()
        .terminal
        .write(b"\r\x1b[2Kalpha changed");
    assert!(!pane.text_match_is_current(text_match));
}

#[test]
fn pane_live_terminal_match_validation_handles_soft_wrapped_matches() {
    let mut terminal = Terminal::new(5, 3, 100).unwrap();
    terminal.write(b"abcdef");
    let pane = PaneTerminal::new(terminal);

    let text_match = pane.search_text_matches("def", true)[0];

    assert_eq!(text_match.start, TerminalTextPoint { row: 0, col: 3 });
    assert_eq!(text_match.end, TerminalTextPoint { row: 1, col: 0 });
    assert!(pane.text_match_is_current(text_match));
}

#[test]
fn pane_live_terminal_match_validation_rejects_an_active_screen_change() {
    let mut terminal = Terminal::new(20, 3, 100).unwrap();
    terminal.write(b"alpha needle");
    let pane = PaneTerminal::new(terminal);

    let text_match = pane.search_text_matches("needle", true)[0];
    pane.core
        .lock()
        .unwrap()
        .terminal
        .write(b"\x1b[?1049hneedle");

    assert!(!pane.text_match_is_current(text_match));
}

#[test]
fn pane_live_terminal_word_motion_expands_across_long_blank_history() {
    let mut terminal = Terminal::new(10, 3, 200).unwrap();
    terminal.write(b"origin\r\n");
    for _ in 0..80 {
        terminal.write(b"\r\n");
    }
    let last_row = u32::try_from(terminal.total_rows().saturating_sub(1)).unwrap();
    let pane = PaneTerminal::new(terminal);

    assert_eq!(
        pane.word_motion_target(last_row, 0, TerminalWordMotion::PreviousStart),
        Some(TerminalTextPoint { row: 0, col: 0 })
    );
}

#[test]
fn pane_live_terminal_word_end_expands_through_a_long_soft_wrap() {
    let mut terminal = Terminal::new(2, 3, 200).unwrap();
    let word = "a".repeat(132);
    terminal.write(word.as_bytes());
    let pane = PaneTerminal::new(terminal);
    let text_match = pane.search_text_matches(&word, true)[0];

    assert_eq!(
        pane.word_motion_target(
            text_match.start.row,
            text_match.start.col,
            TerminalWordMotion::NextEnd,
        ),
        Some(text_match.end)
    );
}

#[test]
fn pane_live_terminal_word_end_expands_through_a_long_wide_soft_wrap() {
    let mut terminal = Terminal::new(2, 3, 200).unwrap();
    let word = "界".repeat(66);
    terminal.write(word.as_bytes());
    let pane = PaneTerminal::new(terminal);
    let text_match = pane.search_text_matches(&word, true)[0];

    // The word end sits on the head cell of the final wide glyph, past the
    // initial read window, so the window has to expand to reach it.
    assert_eq!(
        pane.word_motion_target(
            text_match.start.row,
            text_match.start.col,
            TerminalWordMotion::NextEnd,
        ),
        Some(TerminalTextPoint {
            row: text_match.end.row,
            col: 0,
        })
    );
}
