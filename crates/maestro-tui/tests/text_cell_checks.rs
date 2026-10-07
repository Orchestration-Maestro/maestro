use maestro_tui::text::utils::*;
#[test]
fn width_adapter_covers_base_and_cluster_boundaries() {
    for (text, width) in [
        ("", 0),
        ("ascii", 5),
        ("\tA", 4),
        ("\x1b[31m界\x1b[0m", 2),
        ("\u{301}", 0),
        ("ा", 1),
        ("ᅡ", 0),
        ("ｶﾞ", 1),
        ("☺‍☺", 2),
        ("☀️́", 2),
        ("क्‍ष", 2),
        ("\u{17a4}", 2),
        ("\u{17d8}", 3),
        ("\u{e33}", 1),
        ("ก\u{e33}", 2),
        ("\u{eb3}", 1),
        ("ກ\u{eb3}", 2),
        ("\u{200d}", 0),
        ("\u{1b}", 1),
        ("☀︎", 1),
        ("☀️", 2),
        ("😀", 2),
    ] {
        assert_eq!(visible_width(text), width, "{text:?}");
    }
}

#[test]
fn segmenter_reports_utf16_grapheme_offsets() {
    let input = "a😀e\u{301}";
    let parts: Vec<_> = get_segmenter().segment(input).collect();
    assert_eq!(parts.iter().map(|s| s.index).collect::<Vec<_>>(), [0, 1, 5]);
    assert_eq!(
        parts.iter().map(|s| s.segment).collect::<Vec<_>>(),
        ["a", "😀", "e\u{301}"]
    );
    assert!(parts.iter().all(|s| s.input == input));
    assert_eq!(get_segmenter().segment("").count(), 0);
}

#[test]
fn ansi_extraction_preserves_utf16_and_permissive_boundaries() {
    assert_eq!(
        extract_ansi_code("😀\x1b[31m", 4),
        Some(("\x1b[31m".into(), 5))
    );
    assert_eq!(extract_ansi_code("😀\x1b[31m", 1), None);
    for code in [
        "\x1b[garbageG",
        "\x1b[3K",
        "\x1b[H",
        "\x1b[J",
        "\x1b]title\x07",
        "\x1b]😀\x1b\\",
        "\x1b_apc\x07",
        "\x1b_apc\x1b\\",
    ] {
        assert_eq!(extract_ansi_code(code, 0), Some((code.into(), code.len())));
    }
    for code in [
        "",
        "x",
        "\x1b",
        "\x1b[",
        "\x1b[3A",
        "\x1b]open",
        "\x1b_open",
        "\x1bX",
    ] {
        assert_eq!(extract_ansi_code(code, 0), None);
    }
}

#[test]
fn character_predicates_use_ecmascript_membership() {
    for c in [
        '\t', '\n', '\u{b}', '\u{c}', '\r', ' ', '\u{a0}', '\u{1680}', '\u{2000}', '\u{200a}',
        '\u{2028}', '\u{2029}', '\u{202f}', '\u{205f}', '\u{3000}', '\u{85}',
    ] {
        assert!(is_whitespace_char(&format!("a{c}b")));
    }
    for s in ["", "a", "\u{feff}", "\u{180e}"] {
        assert!(!is_whitespace_char(s));
    }
    for c in "(){}[]<>.,;:'\"!?+-=*/\\|&%^$#@~`".chars() {
        assert!(is_punctuation_char(&format!("a{c}b")));
    }
    for s in ["", "_", "。", "letters"] {
        assert!(!is_punctuation_char(s));
    }
}

#[test]
fn background_style_is_replaceable_and_called_after_padding() {
    let calls = std::cell::RefCell::new(vec![]);
    let first = |s: &str| {
        calls.borrow_mut().push(s.to_owned());
        format!("<{s}>")
    };
    let second = |s: &str| format!("[{s}]");
    assert_eq!(apply_background_to_line("界", 4, &first), "<界  >");
    assert_eq!(*calls.borrow(), ["界  "]);
    assert_eq!(apply_background_to_line("界", 1, &second), "[界]");
    assert_eq!(
        apply_background_to_line("\x1b[31ma", 2, &second),
        "[\x1b[31ma ]"
    );
}

#[test]
fn column_slices_preserve_strictness_and_tab_discrepancy() {
    assert_eq!(slice_with_width("\tA", 0, 1, None), ("\t".into(), 1));
    assert_eq!(visible_width("\tA"), 4);
    assert_eq!(slice_with_width("a界b", 1, 1, None), ("界".into(), 2));
    assert_eq!(slice_with_width("a界b", 1, 1, Some(true)), ("".into(), 0));
    assert_eq!(slice_by_column("a界b", 2, 2, None), "b");
    assert_eq!(
        slice_with_width("\x1b[31mabc\x1b[39m", 1, 2, None),
        ("\x1b[31mbc".into(), 2)
    );
    assert_eq!(slice_by_column("a\x1b[4mbc", 1, 2, None), "\x1b[4mbc");
    assert_eq!(
        slice_with_width("\u{301}a", 0, 1, None),
        ("\u{301}a".into(), 1)
    );
    assert_eq!(slice_by_column("abc", 0, 0, None), "");
}

#[test]
fn segments_preserve_inherited_style_and_overlap_boundaries() {
    assert_eq!(
        extract_segments("\x1b[31mab\x1b[4mcd\x1b[24me", 1, 3, 2, None),
        ("\x1b[31ma".into(), 1, "\x1b[4;31md\x1b[24me".into(), 2)
    );
    assert_eq!(
        extract_segments("a界bc", 2, 1, 4, None),
        ("a界".into(), 3, "bc".into(), 2)
    );
    assert_eq!(
        extract_segments("a界b", 0, 1, 1, Some(true)),
        ("".into(), 0, "".into(), 0)
    );
    assert_eq!(
        extract_segments("\x1b[31mab", 1, 0, 0, None),
        ("\x1b[31ma".into(), 1, "".into(), 0)
    );
    assert_eq!(
        extract_segments("ab", 0, 1, 1, None),
        ("".into(), 0, "b".into(), 1)
    );
    assert_eq!(
        extract_segments("a界b", 0, 2, 2, None),
        ("".into(), 0, "b".into(), 1)
    );
}

#[test]
fn wrapping_preserves_literal_lines_whitespace_and_overwide_graphemes() {
    for (s, w, expected) in [
        ("", 1, vec![""]),
        ("a\n\n", 2, vec!["a", "", ""]),
        ("a  ", 3, vec!["a  "]),
        ("界", 1, vec!["界"]),
        ("a", 0, vec!["a"]),
        ("界a", 0, vec!["界", "a"]),
        ("a界", 1, vec!["a", "界"]),
        ("\u{200b}界", 1, vec!["\u{200b}界"]),
        ("ab\u{feff}", 1, vec!["a", "b\u{feff}"]),
        ("ab\u{85}", 1, vec!["a", "b", ""]),
        ("\x1b[31ma\nb", 2, vec!["\x1b[31ma", "\x1b[31mb"]),
    ] {
        assert_eq!(wrap_text_with_ansi(s, w), expected);
    }
}

#[test]
fn ansi_tracker_preserves_attributes_colors_and_reset_order() {
    let active = |code: &str| extract_segments(&format!("{code}abc"), 0, 1, 1, None).2;
    assert_eq!(
        active("\x1b[9;8;7;5;4;3;2;1;31;44m"),
        "\x1b[1;2;3;4;5;7;8;9;31;44mb"
    );
    for (code, expected) in [
        ("21", "2;3;4;5;7;8;9;31;44"),
        ("22", "3;4;5;7;8;9;31;44"),
        ("23", "1;2;4;5;7;8;9;31;44"),
        ("24", "1;2;3;5;7;8;9;31;44"),
        ("25", "1;2;3;4;7;8;9;31;44"),
        ("27", "1;2;3;4;5;8;9;31;44"),
        ("28", "1;2;3;4;5;7;9;31;44"),
        ("29", "1;2;3;4;5;7;8;31;44"),
        ("39", "1;2;3;4;5;7;8;9;44"),
        ("49", "1;2;3;4;5;7;8;9;31"),
    ] {
        assert_eq!(
            active(&format!("\x1b[1;2;3;4;5;7;8;9;31;44m\x1b[{code}m")),
            format!("\x1b[{expected}mb")
        );
    }
    for reset in ["", "0", "1;0", "00"] {
        assert_eq!(active(&format!("\x1b[31m\x1b[{reset}m")), "b");
    }
    for c in [30, 37, 90, 97, 40, 47, 100, 107] {
        assert_eq!(active(&format!("\x1b[{c}m")), format!("\x1b[{c}mb"));
    }
    for code in ["38;5;007", "48;5;", "38;2;01;02;03", "48;2;1;;3"] {
        assert_eq!(active(&format!("\x1b[{code}m")), format!("\x1b[{code}mb"));
    }
    assert_eq!(active("\x1b[31m\x1b[;99;38;5m"), "\x1b[5;31mb");
    assert_eq!(active("\x1b[00031m\x1b[38;2m"), "\x1b[2;31mb");
    assert_eq!(active("\x1b[31m\x1b[brokenm"), "\x1b[31mb");
    for term in ["\x07", "\x1b\\"] {
        let open = format!("\x1b]8;id=x;https://example.com{term}");
        assert_eq!(active(&format!("{open}\x1b[0m")), format!("{open}b"));
        assert_eq!(active(&format!("{open}\x1b]8;;{term}")), "b");
        assert_eq!(
            active(&format!("{open}\x1b]8;bad{term}")),
            format!("{open}b")
        );
        assert_eq!(
            active(&format!("{open}\x1b]8;p;new{term}")),
            format!("\x1b]8;p;new{term}b")
        );
        assert_eq!(
            wrap_text_with_ansi(&format!("\x1b[4m{open}ab"), 1),
            [
                format!("\x1b[4m{open}a\x1b[24m\x1b]8;;{term}"),
                format!("\x1b[4m{open}b")
            ]
        );
    }
    assert_eq!(active("\x1b]133;A\x07"), "b");
    assert_eq!(active("\x1b[x\x1b[31m"), "\x1b[31mb");
}

#[test]
fn truncation_covers_empty_fractional_and_nonfinite_widths() {
    assert_eq!(truncate_to_width("abc", 0, None, None), "");
    for w in [0, 1, 3, usize::MAX] {
        assert_eq!(truncate_to_width("", w, None, None), "");
    }
    assert_eq!(truncate_to_width("", 3, None, Some(true)), "   ");
    assert_eq!(truncate_to_width("a", usize::MAX, None, None), "a");
    assert_eq!(truncate_to_width("界", usize::MAX, Some("…"), None), "界");
    assert_eq!(
        truncate_to_width("abcdef", 3, None, Some(true)),
        "\x1b[0m...\x1b[0m"
    );
    assert_eq!(truncate_to_width("a", 3, None, Some(true)), "a  ");
    assert_eq!(
        truncate_to_width("abcdef", 1, Some("\u{301}"), Some(true)),
        "a\x1b[0m\u{301}\x1b[0m"
    );
    assert_eq!(truncate_to_width("abcdef", 1, Some("界"), Some(true)), " ");
    assert_eq!(
        truncate_to_width("abcdef", 1, Some("\x1b[31m…"), None),
        "\x1b[0m\x1b[31m…\x1b[0m"
    );
    assert_eq!(
        truncate_to_width("abcdef", 3, Some("\t"), None),
        "\x1b[0m\t\x1b[0m"
    );
    assert_eq!(
        truncate_to_width("\x1b[31ma\x1b[39m", 2, Some("…"), Some(true)),
        "\x1b[31ma\x1b[39m "
    );
}

#[test]
fn truncated_text_preserves_small_width_and_repeat_semantics() {
    use maestro_tui::{Component, TruncatedText};
    for (width, px, py, expected) in [
        (1, 1, 0, vec![" "]),
        (0, 0, 0, vec![""]),
        (3, 0, 2, vec!["   ", "   ", "a  ", "   ", "   "]),
    ] {
        let mut t = TruncatedText::new("a".into(), Some(px), Some(py));
        assert_eq!(t.render(width), expected);
        t.invalidate();
        assert_eq!(t.render(width), expected);
    }
    assert_eq!(
        TruncatedText::new("a".into(), Some(2), None).render(1),
        [" "]
    );
    assert_eq!(
        TruncatedText::new("a".into(), Some(1), None).render(5),
        [" a   "]
    );
}

#[test]
fn terminal_documentation_matches_delivered_examples() {
    use maestro_tui::{Component, Container, TruncatedText};
    use std::{cell::RefCell, rc::Rc};
    let text = include_str!("../../../docs/terminal.md");
    let components = include_str!("../../../docs/terminal/components.md");
    for block in [
        "Use the provided utilities to ensure lines fit:",
        "Both visible_width() and truncate_to_width() correctly handle ANSI escape codes:",
        "visible_width() ignores ANSI codes when calculating width",
        "truncate_to_width() preserves ANSI codes and properly closes them when truncating",
    ] {
        assert!(text.contains(block));
    }
    for block in [
        "All components implement:",
        "Groups child components.",
        "Single-line text that truncates to fit viewport width. Useful for status lines and headers.",
    ] {
        assert!(components.contains(block));
    }
    assert_eq!(visible_width("Hello 界"), 8);
    assert_eq!(
        truncate_to_width("Hello world", 8, None, None),
        "Hello\x1b[0m...\x1b[0m"
    );
    assert_eq!(wrap_text_with_ansi("hello world", 6), ["hello", "world"]);
    let styled = "\x1b[31mHello world\x1b[39m";
    assert_eq!(visible_width(styled), 11);
    assert_eq!(
        truncate_to_width(styled, 8, None, None),
        "\x1b[31mHello\x1b[0m...\x1b[0m"
    );
    let mut c = Container::new();
    c.add_child(Rc::new(RefCell::new(TruncatedText::new(
        "ready".into(),
        Some(1),
        None,
    ))));
    assert_eq!(c.render(8), [" ready  "]);
    c.invalidate();
    c.clear();
    assert!(c.render(8).is_empty());
    assert_eq!(
        TruncatedText::new("Hello world\nignored".into(), None, None).render(8),
        ["Hello\x1b[0m...\x1b[0m"]
    );
}

#[test]
fn column_endpoints_saturate_without_overflow() {
    assert_eq!(
        slice_with_width("abc", 1, usize::MAX, None),
        ("bc".into(), 2)
    );
    assert_eq!(
        extract_segments("abc", 3, usize::MAX, 1, None),
        ("abc".into(), 3, "".into(), 0)
    );
}
