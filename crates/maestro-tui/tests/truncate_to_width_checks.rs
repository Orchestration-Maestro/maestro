use maestro_tui::text::utils::*;
#[test]
fn verify_keeps_output_within_width_for_very_large_unicode_input() {
    let text = "🙂界".repeat(100_000);
    let truncated = truncate_to_width(&text, 40, Some("…"), None);
    assert!(visible_width(&truncated) <= 40);
    assert!(truncated.ends_with("…\x1b[0m"));
}

#[test]
fn verify_preserves_ansi_styling_for_kept_text_and_resets_before_and_after_ellipsis() {
    let text = format!("\x1b[31m{}\x1b[0m", "hello ".repeat(1000));
    let t = truncate_to_width(&text, 20, Some("…"), None);
    assert!(visible_width(&t) <= 20);
    assert!(t.contains("\x1b[31m"));
    assert!(t.ends_with("\x1b[0m…\x1b[0m"));
}

#[test]
fn verify_handles_malformed_ansi_escape_prefixes_without_hanging() {
    let text = format!("abc\x1bnot-ansi {}", "🙂".repeat(1000));
    assert!(visible_width(&truncate_to_width(&text, 20, Some("…"), None)) <= 20);
}

#[test]
fn verify_clips_wide_ellipsis_safely_and_brackets_it_with_resets() {
    assert_eq!(truncate_to_width("abcdef", 1, Some("🙂"), None), "");
    assert_eq!(
        truncate_to_width("abcdef", 2, Some("🙂"), None),
        "\x1b[0m🙂\x1b[0m"
    );
    assert!(visible_width(&truncate_to_width("abcdef", 2, Some("🙂"), None)) <= 2);
}

#[test]
fn verify_returns_the_original_text_when_it_already_fits_even_if_ellipsis_is_too_wide() {
    for t in ["a", "界"] {
        assert_eq!(truncate_to_width(t, 2, Some("🙂"), None), t);
    }
}

#[test]
fn verify_pads_truncated_output_to_requested_width() {
    assert_eq!(
        visible_width(&truncate_to_width("🙂界🙂界🙂界", 8, Some("…"), Some(true))),
        8
    );
}

#[test]
fn verify_adds_a_trailing_reset_when_truncating_without_an_ellipsis() {
    let t = truncate_to_width(
        &format!("\x1b[31m{}", "hello".repeat(100)),
        10,
        Some(""),
        None,
    );
    assert!(visible_width(&t) <= 10);
    assert!(t.ends_with("\x1b[0m"));
}

#[test]
fn verify_keeps_a_contiguous_prefix_instead_of_skipping_a_wide_grapheme_and_resuming_later() {
    assert_eq!(
        truncate_to_width("🙂\t界 \x1b_abc\x07", 7, Some("…"), Some(true)),
        "🙂\t\x1b[0m…\x1b[0m "
    );
}

#[test]
fn verify_counts_tabs_inline_and_skips_ansi_inline() {
    assert_eq!(visible_width("\t\x1b[31m界\x1b[0m"), 5);
}

#[test]
fn verify_keeps_thai_and_lao_am_clusters_at_their_normal_cell_width() {
    for (s, w) in [("ำ", 1), ("ຳ", 1), ("กำ", 2), ("ກຳ", 2)] {
        assert_eq!(visible_width(s), w);
    }
}

#[test]
fn verify_normalizes_thai_and_lao_am_vowels_only_for_terminal_output() {
    assert_eq!(normalize_terminal_output("ำ"), "ํา");
    assert_eq!(normalize_terminal_output("ຳ"), "ໍາ");
    for s in ["ำabc", "ຳabc"] {
        assert_eq!(
            visible_width(&normalize_terminal_output(s)),
            visible_width(s)
        );
    }
}
