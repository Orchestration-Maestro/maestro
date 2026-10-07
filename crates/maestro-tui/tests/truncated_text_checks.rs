use maestro_tui::text::utils::*;
use maestro_tui::{Component, TruncatedText};

#[test]
fn verify_pads_output_lines_to_exactly_match_width() {
    let lines = TruncatedText::new("Hello world".into(), Some(1), Some(0)).render(50);
    assert_eq!(lines.len(), 1);
    assert_eq!(visible_width(&lines[0]), 50);
}

#[test]
fn verify_pads_output_with_vertical_padding_lines_to_width() {
    let lines = TruncatedText::new("Hello".into(), Some(0), Some(2)).render(40);
    assert_eq!(lines.len(), 5);
    for line in lines {
        assert_eq!(visible_width(&line), 40);
    }
}

#[test]
fn verify_truncates_long_text_and_pads_to_width() {
    let lines = TruncatedText::new(
        "This is a very long piece of text that will definitely exceed the available width".into(),
        Some(1),
        Some(0),
    )
    .render(30);
    assert_eq!(lines.len(), 1);
    assert_eq!(visible_width(&lines[0]), 30);
    assert!(lines[0].contains("..."));
}

#[test]
fn verify_preserves_ansi_codes_in_output_and_pads_correctly() {
    let lines = TruncatedText::new(
        "\x1b[31mHello\x1b[39m \x1b[34mworld\x1b[39m".into(),
        Some(1),
        Some(0),
    )
    .render(40);
    assert_eq!(lines.len(), 1);
    assert_eq!(visible_width(&lines[0]), 40);
    assert!(lines[0].contains("\x1b["));
}

#[test]
fn verify_truncates_styled_text_and_adds_reset_code_before_ellipsis() {
    let lines = TruncatedText::new(
        "\x1b[31mThis is a very long red text that will be truncated\x1b[39m".into(),
        Some(1),
        Some(0),
    )
    .render(20);
    assert_eq!(lines.len(), 1);
    assert_eq!(visible_width(&lines[0]), 20);
    assert!(lines[0].contains("\x1b[0m..."));
}

#[test]
fn verify_handles_text_that_fits_exactly() {
    let lines = TruncatedText::new("Hello world".into(), Some(1), Some(0)).render(30);
    assert_eq!(lines.len(), 1);
    assert_eq!(visible_width(&lines[0]), 30);
    assert!(!lines[0].contains("..."));
}

#[test]
fn verify_handles_empty_text() {
    let lines = TruncatedText::new("".into(), Some(1), Some(0)).render(30);
    assert_eq!(lines.len(), 1);
    assert_eq!(visible_width(&lines[0]), 30);
}

#[test]
fn verify_stops_at_newline_and_only_shows_first_line() {
    let lines = TruncatedText::new(
        "First line\nSecond line\nThird line".into(),
        Some(1),
        Some(0),
    )
    .render(40);
    assert_eq!(lines.len(), 1);
    assert_eq!(visible_width(&lines[0]), 40);
    assert!(lines[0].trim().contains("First line"));
    assert!(!lines[0].contains("Second line"));
    assert!(!lines[0].contains("Third line"));
}

#[test]
fn verify_truncates_first_line_even_with_newlines_in_text() {
    let lines = TruncatedText::new(
        "This is a very long first line that needs truncation\nSecond line".into(),
        Some(1),
        Some(0),
    )
    .render(25);
    assert_eq!(lines.len(), 1);
    assert_eq!(visible_width(&lines[0]), 25);
    assert!(lines[0].contains("..."));
    assert!(!lines[0].contains("Second line"));
}
