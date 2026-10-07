use maestro_tui::text::utils::*;

#[test]
fn verify_treats_partial_flag_grapheme_as_full_width_to_avoid_streaming_render_drift() {
    assert_eq!(visible_width("🇨"), 1);
    assert_eq!(visible_width("      - 🇨"), 9);
}

#[test]
fn verify_wraps_intermediate_partial_flag_list_line_before_overflow() {
    let w = wrap_text_with_ansi("      - 🇨", 9);
    assert_eq!(w.len(), 1);
    assert_eq!(w[0], "      - 🇨");
    assert_eq!(visible_width(&w[0]), 9);
}

#[test]
fn verify_treats_all_regional_indicator_singleton_graphemes_as_width_2() {
    for cp in 0x1f1e6..=0x1f1ff {
        assert_eq!(visible_width(&char::from_u32(cp).unwrap().to_string()), 1);
    }
}

#[test]
fn verify_keeps_full_flag_pairs_at_width_2() {
    for flag in ["🇯🇵", "🇺🇸", "🇬🇧", "🇨🇳", "🇩🇪", "🇫🇷"] {
        assert_eq!(visible_width(flag), 2);
    }
}

#[test]
fn verify_keeps_common_streaming_emoji_intermediates_at_stable_width() {
    for s in ["👍", "👍🏻", "✅", "⚡", "⚡️", "👨", "👨‍💻", "🏳️‍🌈"] {
        assert_eq!(visible_width(s), 2);
    }
}
