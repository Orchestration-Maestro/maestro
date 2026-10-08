//! Column selection keeps whole graphemes and closes links at each endpoint.

use maestro_tui::{
    TruncateOptions, extract_segments, slice_by_column, slice_with_width, truncate_to_width,
    visible_width, wrap_text_with_ansi,
};

mod fixtures {
    pub mod cluster_cases;
    pub mod slice_cases;
}
use fixtures::cluster_cases::{CLUSTER_CASES, ClusterCase};
use fixtures::slice_cases::SLICE_CASES;

fn truncate(text: &str, width: usize) -> String {
    truncate_to_width(
        text,
        width,
        TruncateOptions {
            ellipsis: "",
            pad: false,
        },
    )
}

#[test]
fn column_selections_close_links_at_each_endpoint() {
    for terminator in ["\x07", "\x1b\\"] {
        let open = format!("\x1b]8;id=z;https://example.com{terminator}");
        let close = format!("\x1b]8;;{terminator}");
        let slices = [
            (format!("{open}abcd"), format!("{open}a{close}")),
            (format!("{open}a b c"), format!("{open}a{close}")),
            (format!("{open}a\n\nb"), format!("{open}a{close}")),
            (format!("{open}a\x1b[0mb"), format!("{open}a{close}")),
            (format!("{open}\x1b[4mab"), format!("{open}\x1b[4ma{close}")),
            (format!("{open}a{close}b"), format!("{open}a{close}")),
            (open.clone(), String::new()),
        ];
        for (text, expected) in slices {
            let slice = slice_with_width(&text, 0, 1, false);
            assert_eq!(slice.text, expected, "{text:?}");
            assert_eq!(slice_by_column(&text, 0, 1, false), expected);
        }
        let segments = extract_segments(&format!("{open}abcd"), 1, 2, 2, false);
        assert_eq!(segments.before, format!("{open}a{close}"));
        assert_eq!(segments.after, format!("{open}cd{close}"));
        let segments = extract_segments(&format!("{open}a b c"), 1, 2, 2, false);
        assert_eq!(segments.before, format!("{open}a{close}"));
        assert_eq!(segments.after, format!("{open}b {close}"));
        let ended = extract_segments(&format!("{open}ab{close}cd"), 1, 3, 1, false);
        assert_eq!(ended.after, "d", "a link closed in the gap stays closed");
    }
}

#[test]
fn column_slices_keep_whole_graphemes_and_direct_tab_metric() {
    for (text, start, length, strict, expected, width) in SLICE_CASES {
        let slice = slice_with_width(text, *start, *length, *strict);
        assert_eq!(
            (slice.text.as_str(), slice.width),
            (*expected, *width),
            "{text:?} {start} {length} {strict}"
        );
        assert_eq!(slice_by_column(text, *start, *length, *strict), *expected);
    }
    assert_eq!(slice_by_column("abc", usize::MAX, 5, false), "");
    assert_eq!(slice_by_column("abc", 1, usize::MAX, false), "bc");
    assert_eq!(
        slice_by_column("a\u{754c}b", 2, 2, false),
        "b",
        "a grapheme crossing the left edge is omitted"
    );
    assert_eq!(
        slice_by_column("\x1b[31ma\x1b[39m\u{754c}b", 3, 1, false),
        "\x1b[31m\x1b[39mb"
    );
}

/// Scalars of the four cluster families in the tables.
const FAMILIES: [&[char]; 4] = [
    &['\u{1f1e8}', '\u{1f1e6}'],
    &['e', '\u{301}'],
    &['\u{1f44d}', '\u{1f3fd}'],
    &['\u{1f468}', '\u{200d}', '\u{1f4bb}'],
];

/// The text of one family with `escape` before scalar `position`, followed by `X`.
fn with_escape(family: &[char], position: usize) -> String {
    let mut text = String::new();
    for (index, scalar) in family.iter().enumerate() {
        if index == position {
            text.push_str("\x1b[31m");
        }
        text.push(*scalar);
    }
    if position == family.len() {
        text.push_str("\x1b[31m");
    }
    text.push('X');
    text
}

/// Checks every column operation on one text against its expected results.
fn assert_cluster_case(text: &str, width: usize, case: &ClusterCase) {
    assert_eq!(visible_width(text), case.cells, "{text:?}");
    assert_eq!(
        wrap_text_with_ansi(text, width),
        case.wrapped,
        "{text:?} {width}"
    );
    assert_eq!(truncate(text, width), case.truncated, "{text:?} {width}");
    let slice = slice_with_width(text, 0, width, false);
    assert_eq!(
        (slice.text.as_str(), slice.width),
        case.slice,
        "{text:?} {width}"
    );
    let strict = slice_with_width(text, 0, width, true);
    assert_eq!(
        (strict.text.as_str(), strict.width),
        case.strict_slice,
        "{text:?} {width}"
    );
    let parts = extract_segments(text, width, width, 1, true);
    let (before, before_width, after, after_width) = case.segments;
    assert_eq!(
        (
            parts.before.as_str(),
            parts.before_width,
            parts.after.as_str(),
            parts.after_width
        ),
        (before, before_width, after, after_width),
        "{text:?} {width}"
    );
}

/// A piece of output keeps the cluster whole: it holds all of it or none of it.
fn assert_cluster_whole(piece: &str, cluster: &str, text: &str) {
    let visible = strip_escapes(piece);
    let allowed = ["", cluster, "X", &format!("{cluster}X")];
    assert!(
        allowed.contains(&visible.as_str()),
        "{piece:?} splits the cluster in {text:?}"
    );
}

/// Every family, escape position and width in the cluster tables.
fn cluster_inputs() -> Vec<(String, String, usize)> {
    let mut inputs = Vec::new();
    for family in FAMILIES {
        let cluster: String = family.iter().collect();
        for position in 0..=family.len() {
            for width in 0..=4 {
                inputs.push((with_escape(family, position), cluster.clone(), width));
            }
        }
    }
    inputs
}

#[test]
fn maestro_text_keeps_escaped_flag_atomic() {
    let inputs = cluster_inputs();
    assert_eq!(inputs.len(), CLUSTER_CASES.len());
    for (text, cluster, width) in &inputs {
        let case = CLUSTER_CASES
            .iter()
            .find(|case| case.text == *text && case.width == *width)
            .unwrap_or_else(|| panic!("no expected result for {text:?} at {width}"));
        assert_cluster_case(text, *width, case);
        let segments = extract_segments(text, *width, *width, 1, true);
        let pieces = [
            slice_by_column(text, 0, *width, false),
            slice_by_column(text, 0, *width, true),
            segments.before,
            segments.after,
        ];
        for piece in &pieces {
            assert_cluster_whole(piece, cluster, text);
        }
    }
}

/// Text without the `ESC [ 31 m` escape used by the cluster tables.
fn strip_escapes(text: &str) -> String {
    text.replace("\x1b[31m", "")
}

/// Metadata-only text survives every operation unchanged.
fn assert_metadata_only_text() {
    let marker = "\x1b_maestro:c\x07";
    let prompt = "\x1b]133;A\x07";
    for text in [marker, prompt, "a\x1b[2K", &format!("{marker}\n")] {
        let expected_last = if text.ends_with('\n') { "" } else { text };
        assert_eq!(
            wrap_text_with_ansi(text, 0).last().map(String::as_str),
            Some(expected_last)
        );
        assert_eq!(truncate(text, 3), text);
        assert_eq!(slice_by_column(text, 0, 3, false), text);
    }
    assert_eq!(wrap_text_with_ansi(marker, 5), [marker]);
    assert_eq!(wrap_text_with_ansi(&format!("{marker}\n"), 5), [marker, ""]);
    assert_eq!(wrap_text_with_ansi("a\x1b[2K", 0), ["a\x1b[2K"]);
    assert_eq!(slice_with_width(marker, 0, 3, false).width, 0);
}

/// Style escapes alone never form output, while metadata beside them stays.
fn assert_style_only_text_vanishes() {
    let marker = "\x1b_maestro:c\x07";
    let styled = format!("\x1b[31m{marker}");
    assert_eq!(wrap_text_with_ansi(&styled, 0), [marker]);
    assert_eq!(truncate(&styled, 3), marker);
    assert_eq!(slice_with_width(&styled, 0, 3, false).text, marker);
    assert_eq!(wrap_text_with_ansi("a\n", 5), ["a", ""]);
    assert_eq!(wrap_text_with_ansi("\x1b[31m\n", 5), ["", ""]);
}

#[test]
fn opaque_metadata_survives_without_printable_content() {
    assert_metadata_only_text();
    assert_style_only_text_vanishes();
    let after_content = "ab\x1b]133;B\x07";
    assert_eq!(wrap_text_with_ansi(after_content, 2), [after_content]);
    assert_eq!(
        wrap_text_with_ansi(after_content, 1),
        ["a", "b\x1b]133;B\x07"]
    );
    assert_eq!(
        wrap_text_with_ansi("ab\x1b]133;A\x07 cd", 2),
        ["ab\x1b]133;A\x07", "cd"]
    );
}

#[test]
fn segments_keep_before_priority_and_gap_style_changes() {
    let cases = [
        ("a\u{754c}bc", (2, 1, 4, false), ("a\u{754c}", 3, "bc", 2)),
        ("abcd", (0, 2, 0, false), ("", 0, "", 0)),
        ("abcd", (3, 0, 1, false), ("a", 1, "", 0)),
        (
            "\x1b[31mab\x1b[1;44mcd\x1b[39mef",
            (1, 4, 4, false),
            ("\x1b[31ma", 1, "\x1b[1;44mef", 2),
        ),
        ("a\u{754c}b", (0, 1, 1, true), ("", 0, "", 0)),
        ("a\u{754c}b", (0, 1, 1, false), ("", 0, "\u{754c}", 2)),
        ("\t\u{754c}b", (1, 1, 2, true), ("\t\u{754c}", 2, "b", 1)),
        ("ab\x1b[31mcd", (1, 2, 2, false), ("a", 1, "\x1b[31mcd", 2)),
        ("ab\x1b[31mcd", (2, 3, 1, false), ("ab", 2, "\x1b[31md", 1)),
    ];
    for (text, (before_end, after_start, after_len, strict), expected) in cases {
        let parts = extract_segments(text, before_end, after_start, after_len, strict);
        let (before, before_width, after, after_width) = expected;
        assert_eq!(
            (
                parts.before.as_str(),
                parts.before_width,
                parts.after.as_str(),
                parts.after_width
            ),
            (before, before_width, after, after_width),
            "{text:?}"
        );
    }
}
