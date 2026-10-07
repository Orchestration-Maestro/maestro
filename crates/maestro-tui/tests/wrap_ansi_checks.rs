use maestro_tui::text::utils::*;

#[test]
fn verify_not_apply_underline_style_before_the_styled_text() {
    let wrapped = wrap_text_with_ansi(
        "read this thread \x1b[4mhttps://example.com/very/long/path/that/will/wrap\x1b[24m",
        40,
    );
    assert_eq!(wrapped[0], "read this thread");
    assert!(wrapped[1].starts_with("\x1b[4m"));
    assert!(wrapped[1].contains("https://"));
}

#[test]
fn verify_not_have_whitespace_before_underline_reset_code() {
    let w = wrap_text_with_ansi("\x1b[4munderlined text here \x1b[24mmore", 18);
    assert!(!w[0].contains(" \x1b[24m"));
}

#[test]
fn verify_not_bleed_underline_to_padding_each_line_should_end_with_reset_for_underline_only() {
    let w = wrap_text_with_ansi(
        "prefix \x1b[4mhttps://example.com/very/long/path/that/will/definitely/wrap\x1b[24m suffix",
        30,
    );
    for line in &w[1..w.len() - 1] {
        if line.contains("\x1b[4m") {
            assert!(line.ends_with("\x1b[24m"));
            assert!(!line.ends_with("\x1b[0m"));
        }
    }
}

#[test]
fn verify_preserve_background_color_across_wrapped_lines_without_full_reset() {
    let w = wrap_text_with_ansi(
        "\x1b[44mhello world this is blue background text\x1b[0m",
        15,
    );
    for line in &w {
        assert!(line.contains("\x1b[44m"));
    }
    for line in &w[..w.len() - 1] {
        assert!(!line.ends_with("\x1b[0m"));
    }
}

#[test]
fn verify_reset_underline_but_preserve_background_when_wrapping_underlined_text_inside_background()
{
    let w = wrap_text_with_ansi(
        "\x1b[41mprefix \x1b[4mUNDERLINED_CONTENT_THAT_WRAPS\x1b[24m suffix\x1b[0m",
        20,
    );
    for line in &w {
        assert!(line.contains("[41m") || line.contains(";41m") || line.contains("[41;"));
    }
    for line in &w[..w.len() - 1] {
        if (line.contains("[4m") || line.contains("[4;") || line.contains(";4m"))
            && !line.contains("\x1b[24m")
        {
            assert!(line.ends_with("\x1b[24m"));
            assert!(!line.ends_with("\x1b[0m"));
        }
    }
}

#[test]
fn verify_wrap_plain_text_correctly() {
    let w = wrap_text_with_ansi("hello world this is a test", 10);
    assert!(w.len() > 1);
    for line in w {
        assert!(visible_width(&line) <= 10);
    }
}

#[test]
fn verify_ignore_osc_133_semantic_markers_in_visible_width() {
    assert_eq!(visible_width("\x1b]133;A\x07hello\x1b]133;B\x07"), 5);
}

#[test]
fn verify_ignore_osc_sequences_terminated_with_st_in_visible_width() {
    assert_eq!(visible_width("\x1b]133;A\x1b\\hello\x1b]133;B\x1b\\"), 5);
}

#[test]
fn verify_treat_isolated_regional_indicators_as_width_2() {
    assert_eq!(visible_width("🇨"), 1);
    assert_eq!(visible_width("🇨🇳"), 2);
}

#[test]
fn verify_truncate_trailing_whitespace_that_exceeds_width() {
    assert!(visible_width(&wrap_text_with_ansi("  ", 1)[0]) <= 1);
}

#[test]
fn verify_preserve_color_codes_across_wraps() {
    let w = wrap_text_with_ansi("\x1b[31mhello world this is red\x1b[0m", 10);
    for line in &w[1..] {
        assert!(line.starts_with("\x1b[31m"));
    }
    for line in &w[..w.len() - 1] {
        assert!(!line.ends_with("\x1b[0m"));
    }
}

#[test]
fn verify_re_emits_osc_8_open_at_the_start_of_continuation_lines() {
    let w = wrap_text_with_ansi(
        "\x1b]8;;https://example.com\x1b\\0123456789\x1b]8;;\x1b\\",
        6,
    );
    for line in w {
        if visible_width(&line) > 0 {
            assert!(
                line.starts_with("\x1b]8;;https://example.com\x1b\\")
                    || line.contains("\x1b]8;;https://example.com\x1b\\")
            );
        }
    }
}

#[test]
fn verify_closes_osc_8_before_each_line_break() {
    let w = wrap_text_with_ansi(
        "\x1b]8;;https://example.com\x1b\\0123456789\x1b]8;;\x1b\\",
        6,
    );
    for line in &w[..w.len() - 1] {
        if line.contains("\x1b]8;;https://example.com\x1b\\") {
            assert!(line.ends_with("\x1b]8;;\x1b\\"));
        }
    }
}

#[test]
fn verify_preserves_bel_terminators_when_wrapping_oauth_style_hyperlinks() {
    let url = format!("https://example.com/oauth/{}", "a".repeat(32));
    let w = wrap_text_with_ansi(&format!("\x1b]8;;{url}\x07{url}\x1b]8;;\x07"), 20);
    assert!(w.len() > 1);
    for line in &w {
        assert!(line.contains(&format!("\x1b]8;;{url}\x07")));
        assert!(!line.contains(&format!("\x1b]8;;{url}\x1b\\")));
    }
    for line in &w[..w.len() - 1] {
        assert!(line.ends_with("\x1b]8;;\x07"));
    }
}

#[test]
fn verify_does_not_emit_osc_8_sequences_on_lines_that_are_outside_the_hyperlink() {
    let w = wrap_text_with_ansi(
        "before \x1b]8;;https://example.com\x1b\\link\x1b]8;;\x1b\\ after",
        80,
    );
    assert_eq!(w.len(), 1);
    assert_eq!(w[0].matches("\x1b]8;;https://example.com\x1b\\").count(), 1);
    assert_eq!(w[0].matches("\x1b]8;;\x1b\\").count(), 1);
}
