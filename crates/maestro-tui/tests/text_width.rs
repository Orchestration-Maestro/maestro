//! Visible width, character classes, segmentation and escape extraction.

use maestro_tui::{
    apply_background_to_line, extract_ansi_code, get_segmenter, is_punctuation_char,
    is_whitespace_char, normalize_terminal_output, visible_width, wrap_text_with_ansi,
};

/// Measured inputs with their exact cell counts.
const WIDTH_POLICY: &[(&str, usize)] = &[
    ("", 0),
    ("ASCII ~", 7),
    ("\u{754c}", 2),
    ("\u{1f642}", 2),
    ("e\u{301}", 1),
    ("\u{301}", 0),
    ("\u{93e}", 0),
    ("\u{600}", 0),
    ("\u{1160}", 0),
    ("\u{17a4}", 1),
    ("\u{17d8}", 1),
    ("\u{115f}", 0),
    ("\u{2028}", 1),
    ("\u{e01}\u{e33}", 2),
    ("\u{e81}\u{eb3}", 2),
    ("\u{1f468}\u{200d}\u{1f4bb}", 2),
    ("\u{1f3f3}\u{fe0f}\u{200d}\u{1f308}", 2),
    ("\u{1f468}\u{200d}\u{1f469}", 2),
    ("a\u{93e}", 1),
    ("\u{34f}", 0),
    ("\u{26a1}\u{fe0e}", 2),
    ("\u{915}\u{94d}\u{937}", 1),
    ("a\u{200d}", 1),
    ("\u{e4d}\u{e32}", 1),
    ("\u{0}", 0),
    ("\r\n", 0),
    ("\u{7f}", 0),
    ("\u{85}", 0),
    ("\t", 3),
    ("\u{1161}", 1),
    ("\u{ff9e}", 1),
    ("\u{ff76}\u{ff9e}", 2),
    ("\u{a9}", 1),
    ("\u{a9}\u{fe0f}", 2),
];

/// Escape inputs, the sequence recognized at byte zero and the visible width.
/// A recognized escape: its text and byte length.
type Found = (&'static str, usize);

const EXTRACTION: &[(&str, Option<Found>, usize)] = &[
    ("\u{1b}[31m", Some(("\u{1b}[31m", 5)), 0),
    ("\u{1b}[1G", Some(("\u{1b}[1G", 4)), 0),
    ("\u{1b}[K", Some(("\u{1b}[K", 3)), 0),
    ("\u{1b}[H", Some(("\u{1b}[H", 3)), 0),
    ("\u{1b}[J", Some(("\u{1b}[J", 3)), 0),
    ("\u{1b}]8;;url\u{7}", Some(("\u{1b}]8;;url\u{7}", 9)), 0),
    (
        "\u{1b}]8;;url\u{1b}\\",
        Some(("\u{1b}]8;;url\u{1b}\\", 10)),
        0,
    ),
    (
        "\u{1b}_maestro:c\u{7}",
        Some(("\u{1b}_maestro:c\u{7}", 12)),
        0,
    ),
    ("\u{1b}_abc\u{1b}\\", Some(("\u{1b}_abc\u{1b}\\", 7)), 0),
    ("\u{1b}]133;A\u{7}", Some(("\u{1b}]133;A\u{7}", 8)), 0),
    ("\u{1b}", None, 0),
    ("\u{1b}[31", None, 3),
    ("\u{1b}[?25l", None, 5),
    ("\u{1b}]8;;url", None, 7),
    ("\u{1b}_abc", None, 4),
    ("\u{1b}Pignored", None, 8),
    (
        "\u{1b}[bad\u{1b}[31m",
        Some(("\u{1b}[bad\u{1b}[31m", 10)),
        0,
    ),
    ("\u{1b}[31x99m", Some(("\u{1b}[31x99m", 8)), 0),
];

#[test]
fn unicode_width_policy_covers_marks_formats_and_variants() {
    for (text, cells) in WIDTH_POLICY {
        assert_eq!(visible_width(text), *cells, "{text:?}");
    }
}

#[test]
fn segmentation_reports_utf8_text_positions() {
    let segments = |text| get_segmenter(text).collect::<Vec<_>>();
    assert_eq!(
        segments("a\u{754c}\u{1f642}e\u{301}"),
        [(0, "a"), (1, "\u{754c}"), (4, "\u{1f642}"), (8, "e\u{301}")]
    );
    for whole in [
        "\u{1f1e8}\u{1f1e6}",
        "\u{1f44d}\u{1f3fd}",
        "\u{1f468}\u{200d}\u{1f4bb}",
        "\r\n",
        "\u{915}\u{94d}\u{937}",
    ] {
        assert_eq!(segments(whole), [(0, whole)]);
    }
    assert_eq!(segments(""), []);
}

#[test]
fn character_classes_keep_bom_and_next_line_distinct() {
    let whitespace = [
        '\t', '\n', '\u{b}', '\u{c}', '\r', ' ', '\u{a0}', '\u{1680}', '\u{2000}', '\u{2001}',
        '\u{2002}', '\u{2003}', '\u{2004}', '\u{2005}', '\u{2006}', '\u{2007}', '\u{2008}',
        '\u{2009}', '\u{200a}', '\u{2028}', '\u{2029}', '\u{202f}', '\u{205f}', '\u{3000}',
        '\u{feff}',
    ];
    assert_eq!(whitespace.len(), 25);
    for scalar in whitespace {
        assert!(is_whitespace_char(&scalar.to_string()), "{scalar:?}");
    }
    for scalar in ['\u{85}', '\u{200b}', '\u{180e}', 'a', '.'] {
        assert!(!is_whitespace_char(&scalar.to_string()), "{scalar:?}");
    }
    let punctuation = "(){}[]<>.,;:'\"!?+-=*/\\|&%^$#@~`";
    assert_eq!(punctuation.chars().count(), 31);
    for scalar in punctuation.chars() {
        assert!(is_punctuation_char(&scalar.to_string()), "{scalar:?}");
    }
    for scalar in ['a', '_', ' ', '\u{2026}', '\u{3002}'] {
        assert!(!is_punctuation_char(&scalar.to_string()), "{scalar:?}");
    }
    assert!(is_whitespace_char("ab c") && !is_whitespace_char("abc"));
    assert!(is_punctuation_char("ab.") && !is_punctuation_char("abc"));
    assert!(!is_whitespace_char("") && !is_punctuation_char(""));
}

#[test]
fn escape_extraction_keeps_supported_and_incomplete_sequences() {
    for (text, code, cells) in EXTRACTION {
        let found = extract_ansi_code(text, 0).map(|found| (found.code, found.length));
        assert_eq!(found, *code, "{text:?}");
        assert_eq!(visible_width(text), *cells, "{text:?}");
    }
    let text = "\u{1f642}\x1b[31m";
    assert_eq!(
        extract_ansi_code(text, 4).map(|found| found.code),
        Some("\x1b[31m")
    );
    for position in [0, 1, 2, 3, 99] {
        assert!(extract_ansi_code(text, position).is_none(), "{position}");
    }
}

#[test]
fn background_callback_observes_unclipped_padded_content() {
    let cases = [
        ("a", 4, "<a   >", "a   "),
        ("\u{754c}", 1, "<\u{754c}>", "\u{754c}"),
        ("", 0, "<>", ""),
        (
            "\x1b[31ma\x1b[0m",
            3,
            "<\x1b[31ma\x1b[0m  >",
            "\x1b[31ma\x1b[0m  ",
        ),
    ];
    for (line, width, output, seen) in cases {
        let mut calls = Vec::new();
        let result = apply_background_to_line(line, width, |text| {
            calls.push(text.to_owned());
            format!("<{text}>")
        });
        assert_eq!(result, output);
        assert_eq!(calls, [seen]);
    }
}

#[test]
fn regional_indicators_keep_two_cells() {
    for scalar in '\u{1f1e6}'..='\u{1f1ff}' {
        assert_eq!(visible_width(&scalar.to_string()), 2, "{scalar:?}");
    }
    assert_eq!(visible_width("\u{1f1e8}"), 2);
    assert_eq!(visible_width("\u{1f1e8}\u{1f1f3}"), 2);
}

#[test]
fn partial_flag_list_wraps_before_overflow() {
    let list_line = "      - \u{1f1e8}";
    assert_eq!(visible_width(list_line), 10);
    let wrapped = wrap_text_with_ansi(list_line, 9);
    assert_eq!(wrapped.len(), 2);
    assert_eq!(visible_width(&wrapped[0]), 7);
    assert_eq!(visible_width(&wrapped[1]), 2);
}

#[test]
fn paired_flags_keep_two_cells() {
    for flag in [
        "\u{1f1ef}\u{1f1f5}",
        "\u{1f1fa}\u{1f1f8}",
        "\u{1f1ec}\u{1f1e7}",
        "\u{1f1e8}\u{1f1f3}",
        "\u{1f1e9}\u{1f1ea}",
        "\u{1f1eb}\u{1f1f7}",
    ] {
        assert_eq!(visible_width(flag), 2, "{flag}");
    }
}

#[test]
fn streaming_emoji_keep_stable_cells() {
    for sample in [
        "\u{1f44d}",
        "\u{1f44d}\u{1f3fb}",
        "\u{2705}",
        "\u{26a1}",
        "\u{26a1}\u{fe0f}",
        "\u{1f468}",
        "\u{1f468}\u{200d}\u{1f4bb}",
        "\u{1f3f3}\u{fe0f}\u{200d}\u{1f308}",
    ] {
        assert_eq!(visible_width(sample), 2, "{sample}");
    }
}

#[test]
fn am_clusters_keep_authored_cell_counts() {
    assert_eq!(visible_width("\u{e33}"), 1);
    assert_eq!(visible_width("\u{eb3}"), 1);
    assert_eq!(visible_width("\u{e01}\u{e33}"), 2);
    assert_eq!(visible_width("\u{e81}\u{eb3}"), 2);
}

#[test]
fn terminal_normalization_changes_only_am_vowels() {
    let cases = [
        ("\u{e33}", "\u{e4d}\u{e32}"),
        ("\u{eb3}", "\u{ecd}\u{eb2}"),
        ("\u{e33}abc", "\u{e4d}\u{e32}abc"),
        ("\u{eb3}abc", "\u{ecd}\u{eb2}abc"),
        (
            "\u{e33}\u{eb3}x\u{e01}\u{e33}\u{e81}\u{eb3}",
            "\u{e4d}\u{e32}\u{ecd}\u{eb2}x\u{e01}\u{e4d}\u{e32}\u{e81}\u{ecd}\u{eb2}",
        ),
        (
            "\x1b[31m\u{e01}\u{e33}\x1b[0m",
            "\x1b[31m\u{e01}\u{e4d}\u{e32}\x1b[0m",
        ),
        ("", ""),
        ("plain", "plain"),
    ];
    for (text, normalized) in cases {
        assert_eq!(normalize_terminal_output(text), normalized);
        assert_eq!(
            visible_width(&normalize_terminal_output(text)),
            visible_width(text)
        );
    }
    assert!(matches!(
        normalize_terminal_output("plain"),
        std::borrow::Cow::Borrowed(_)
    ));
}

#[test]
fn semantic_bel_markers_have_no_cells() {
    assert_eq!(visible_width("\x1b]133;A\x07hello\x1b]133;B\x07"), 5);
}

#[test]
fn semantic_st_markers_have_no_cells() {
    assert_eq!(visible_width("\x1b]133;A\x1b\\hello\x1b]133;B\x1b\\"), 5);
}

#[test]
fn tabs_and_inline_escapes_have_distinct_widths() {
    assert_eq!(visible_width("\t\x1b[31m\u{754c}\x1b[0m"), 5);
}
