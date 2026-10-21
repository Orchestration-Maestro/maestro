//! Dynamic and authored status text retain their distinct sanitation policies.

use super::*;

#[test]
fn daemon_command_output_uses_sanitized_last_line() {
    assert_eq!(
        command_output_text(b"old\n win\x1b[31mter\r\n"),
        Some("win[31mter".into())
    );
    assert_eq!(command_output_text(b"\r\n"), None);
}

#[test]
fn daemon_status_text_strips_bidi_and_zero_width_format_controls() {
    assert_eq!(
        sanitize_status_text("safe\u{202e}evil\u{200b}"),
        Some("safeevil".into())
    );
}

#[test]
fn daemon_separator_preserves_printable_spacing_and_drops_controls() {
    assert_eq!(sanitize_separator(" \x1b|\n "), " | ");
    assert_eq!(sanitize_literal_text(" \x1b|\n "), Some(" | ".into()));
}
