//! Greedy wrapping that keeps style and hyperlink state across line breaks.

use maestro_tui::{visible_width, wrap_text_with_ansi};

mod fixtures {
    pub mod terminal;
}
use fixtures::terminal::{replay, replay_lines};

const UNDERLINE_ON: &str = "\x1b[4m";
const UNDERLINE_OFF: &str = "\x1b[24m";
const RESET: &str = "\x1b[0m";
const RED: &str = "\x1b[31m";
const MARKER: &str = "\x1b_maestro:c\x07";

#[test]
fn underline_begins_with_its_content() {
    let url = "https://example.com/very/long/path/that/will/wrap";
    let text = format!("read this thread {UNDERLINE_ON}{url}{UNDERLINE_OFF}");
    let wrapped = wrap_text_with_ansi(&text, 40);
    assert_eq!(
        wrapped,
        [
            "read this thread".to_owned(),
            format!("{UNDERLINE_ON}https://example.com/very/long/path/that/{UNDERLINE_OFF}"),
            format!("{UNDERLINE_ON}will/wrap{UNDERLINE_OFF}"),
        ]
    );
}

#[test]
fn overflow_spaces_precede_no_underline_reset() {
    let text = format!("{UNDERLINE_ON}underlined text here {UNDERLINE_OFF}more");
    assert_eq!(
        wrap_text_with_ansi(&text, 18),
        [
            format!("{UNDERLINE_ON}underlined text{UNDERLINE_OFF}"),
            format!("{UNDERLINE_ON}here {UNDERLINE_OFF}more"),
        ]
    );
}

#[test]
fn soft_wrap_closes_underline_without_full_reset() {
    let url = "https://example.com/very/long/path/that/will/definitely/wrap";
    let text = format!("prefix {UNDERLINE_ON}{url}{UNDERLINE_OFF} suffix");
    let wrapped = wrap_text_with_ansi(&text, 30);
    assert_eq!(
        wrapped,
        [
            "prefix".to_owned(),
            format!("{UNDERLINE_ON}https://example.com/very/long/{UNDERLINE_OFF}"),
            format!("{UNDERLINE_ON}path/that/will/definitely/wrap{UNDERLINE_OFF}"),
            "suffix".to_owned(),
        ]
    );
    assert!(wrapped.iter().all(|line| !line.ends_with(RESET)));
}

#[test]
fn background_survives_soft_wrap() {
    let text = format!("\x1b[44mhello world this is blue background text{RESET}");
    assert_eq!(
        wrap_text_with_ansi(&text, 15),
        [
            "\x1b[44mhello world".to_owned(),
            "\x1b[44mthis is blue".to_owned(),
            format!("\x1b[44mbackground text{RESET}"),
        ]
    );
}

#[test]
fn underlined_background_keeps_selective_resets() {
    let text = format!(
        "\x1b[41mprefix {UNDERLINE_ON}UNDERLINED_CONTENT_THAT_WRAPS{UNDERLINE_OFF} suffix{RESET}"
    );
    assert_eq!(
        wrap_text_with_ansi(&text, 20),
        [
            "\x1b[41mprefix".to_owned(),
            format!("\x1b[41m{UNDERLINE_ON}UNDERLINED_CONTENT_T{UNDERLINE_OFF}"),
            format!("\x1b[4;41mHAT_WRAPS{UNDERLINE_OFF} suffix{RESET}"),
        ]
    );
}

#[test]
fn plain_words_wrap_greedily() {
    let wrapped = wrap_text_with_ansi("hello world this is a test", 10);
    assert_eq!(wrapped, ["hello", "world this", "is a test"]);
    assert!(wrapped.iter().all(|line| visible_width(line) <= 10));
}

#[test]
fn overflow_trailing_spaces_do_not_exceed_width() {
    assert_eq!(wrap_text_with_ansi("ab   ", 2), ["ab"]);
}

#[test]
fn foreground_carries_across_soft_wraps() {
    let text = format!("\x1b[31mhello world this is red{RESET}");
    assert_eq!(
        wrap_text_with_ansi(&text, 10),
        [
            "\x1b[31mhello".to_owned(),
            "\x1b[31mworld this".to_owned(),
            format!("{RED}is red{RESET}"),
        ]
    );
}

#[test]
fn literal_newlines_carry_style_and_leave_underline_open() {
    assert_eq!(
        wrap_preserving_styles(&format!("{RED}a\nb"), 5),
        [format!("{RED}a"), format!("{RED}b")]
    );
    assert_eq!(
        wrap_preserving_styles(&format!("{UNDERLINE_ON}ab\ncd"), 5),
        [format!("{UNDERLINE_ON}ab"), format!("{UNDERLINE_ON}cd")]
    );
}

#[test]
fn wrapped_st_links_close_and_reopen() {
    let open = "\x1b]8;;https://example.com\x1b\\";
    let close = "\x1b]8;;\x1b\\";
    let input = format!("{open}0123456789{close}");
    assert_eq!(
        wrap_text_with_ansi(&input, 6),
        [format!("{open}012345{close}"), format!("{open}6789{close}")]
    );
}

#[test]
fn wrapped_bel_links_keep_bel_terminators() {
    let url = format!("https://example.com/oauth/{}", "a".repeat(32));
    let open = format!("\x1b]8;;{url}\x07");
    let close = "\x1b]8;;\x07";
    let lines = wrap_text_with_ansi(&format!("{open}{url}{close}"), 20);
    assert_eq!(
        lines,
        [
            format!("{open}https://example.com/{close}"),
            format!("{open}oauth/aaaaaaaaaaaaaa{close}"),
            format!("{open}aaaaaaaaaaaaaaaaaa{close}"),
        ]
    );
}

#[test]
fn text_outside_links_gets_no_extra_link_codes() {
    let input = "before \x1b]8;;https://example.com\x1b\\link\x1b]8;;\x1b\\ after";
    assert_eq!(wrap_text_with_ansi(input, 80), [input]);
}

/// Wraps `text` and checks that every line, replayed on a fresh terminal, styles its
/// visible non-whitespace scalars exactly as the source does.
fn wrap_preserving_styles(text: &str, width: usize) -> Vec<String> {
    let lines = wrap_text_with_ansi(text, width);
    let source: Vec<_> = replay(text)
        .into_iter()
        .filter(|(scalar, _)| !scalar.is_whitespace())
        .collect();
    assert_eq!(
        replay_lines(&lines),
        source,
        "{text:?} at {width}: {lines:?}"
    );
    lines
}

#[test]
fn wrapping_retains_logical_lines_and_fitting_spaces() {
    let cases: &[(&str, usize, &[&str])] = &[
        ("", 0, &[""]),
        ("", 5, &[""]),
        ("\n", 0, &["", ""]),
        ("\n", 5, &["", ""]),
        ("a\n\nb\n", 0, &["a", "", "b", ""]),
        ("a\n\nb\n", 1, &["a", "", "b", ""]),
        ("a\n\nb\n", 5, &["a", "", "b", ""]),
        ("  ", 0, &[""]),
        ("  ", 1, &[""]),
        ("  ", 2, &["  "]),
        ("  ", 5, &["  "]),
        ("a ", 0, &["a"]),
        ("a ", 1, &["a"]),
        ("a ", 2, &["a "]),
        ("a\t", 0, &["a"]),
        ("a\t", 1, &["a"]),
        ("a\t", 4, &["a\t"]),
        ("a\t", 5, &["a\t"]),
        ("a\tb", 4, &["a", "b"]),
        ("a\tb", 5, &["a\tb"]),
        ("a  b", 5, &["a  b"]),
        ("a   b", 3, &["a", "b"]),
        ("a\u{feff} b", 0, &["a", "b"]),
        ("a\u{feff} b", 1, &["a", "b"]),
        ("a\u{feff} b", 5, &["a\u{feff} b"]),
        ("a\u{85} b", 1, &["a\u{85}", "b"]),
        ("a\u{85} b", 5, &["a\u{85} b"]),
        ("ab cd", 4, &["ab", "cd"]),
        ("ab\n  cd", 3, &["ab", "cd"]),
    ];
    for (text, width, expected) in cases {
        assert_eq!(
            wrap_text_with_ansi(text, *width),
            *expected,
            "{text:?} at {width}"
        );
    }
}

#[test]
fn maestro_wrap_discards_style_only_overflow() {
    let cases: &[(&str, usize, &[&str])] = &[
        ("a\x1b[31m \x1b[39mb", 0, &["a", "\x1b[31m\x1b[39mb"]),
        ("a\x1b[31m \x1b[39mb", 1, &["a", "\x1b[31m\x1b[39mb"]),
        (
            "a\x1b[31m \x1b[39mb",
            2,
            &["a\x1b[31m", "\x1b[31m\x1b[39mb"],
        ),
        ("\x1b[31ma ", 0, &["\x1b[31ma"]),
        ("\x1b[31ma ", 1, &["\x1b[31ma"]),
        ("\x1b[31ma ", 2, &["\x1b[31ma "]),
        ("\x1b[31m\u{754c}", 0, &["\x1b[31m\u{754c}"]),
        ("\x1b[31m\u{754c}", 1, &["\x1b[31m\u{754c}"]),
        ("\x1b[31m\u{754c}", 2, &["\x1b[31m\u{754c}"]),
        ("a \x1b[31m ", 1, &["a"]),
        ("a \x1b[31m ", 2, &["a"]),
        ("\x1b[31m\n\n", 0, &["", "", ""]),
        ("\x1b[31m\n\n", 1, &["", "", ""]),
        ("\x1b[31m\n\n", 2, &["", "", ""]),
        ("\u{754c}\u{301}a", 1, &["\u{754c}\u{301}", "a"]),
        ("\u{301}\u{754c}", 1, &["\u{301}\u{754c}"]),
        ("\u{754c}\u{85}x", 1, &["\u{754c}\u{85}", "x"]),
        ("ab\x1b[31m  \x1b[0m", 2, &["ab\x1b[0m"]),
    ];
    for (text, width, expected) in cases {
        assert_eq!(
            wrap_preserving_styles(text, *width),
            *expected,
            "{text:?} at {width}"
        );
    }
}

const CONTROLS: &[(&str, &str)] = &[
    ("\u{1b}[1m", "\u{1b}[1;2;3;4;5;7;8;9;31;44m"),
    ("\u{1b}[2m", "\u{1b}[1;2;3;4;5;7;8;9;31;44m"),
    ("\u{1b}[3m", "\u{1b}[1;2;3;4;5;7;8;9;31;44m"),
    ("\u{1b}[4m", "\u{1b}[1;2;3;4;5;7;8;9;31;44m"),
    ("\u{1b}[5m", "\u{1b}[1;2;3;4;5;7;8;9;31;44m"),
    ("\u{1b}[7m", "\u{1b}[1;2;3;4;5;7;8;9;31;44m"),
    ("\u{1b}[8m", "\u{1b}[1;2;3;4;5;7;8;9;31;44m"),
    ("\u{1b}[9m", "\u{1b}[1;2;3;4;5;7;8;9;31;44m"),
    ("\u{1b}[21m", "\u{1b}[2;3;4;5;7;8;9;31;44m"),
    ("\u{1b}[22m", "\u{1b}[3;4;5;7;8;9;31;44m"),
    ("\u{1b}[23m", "\u{1b}[1;2;4;5;7;8;9;31;44m"),
    ("\u{1b}[24m", "\u{1b}[1;2;3;5;7;8;9;31;44m"),
    ("\u{1b}[25m", "\u{1b}[1;2;3;4;7;8;9;31;44m"),
    ("\u{1b}[27m", "\u{1b}[1;2;3;4;5;8;9;31;44m"),
    ("\u{1b}[28m", "\u{1b}[1;2;3;4;5;7;9;31;44m"),
    ("\u{1b}[29m", "\u{1b}[1;2;3;4;5;7;8;31;44m"),
    ("\u{1b}[30m", "\u{1b}[1;2;3;4;5;7;8;9;30;44m"),
    ("\u{1b}[37m", "\u{1b}[1;2;3;4;5;7;8;9;37;44m"),
    ("\u{1b}[90m", "\u{1b}[1;2;3;4;5;7;8;9;90;44m"),
    ("\u{1b}[97m", "\u{1b}[1;2;3;4;5;7;8;9;97;44m"),
    ("\u{1b}[40m", "\u{1b}[1;2;3;4;5;7;8;9;31;40m"),
    ("\u{1b}[47m", "\u{1b}[1;2;3;4;5;7;8;9;31;47m"),
    ("\u{1b}[100m", "\u{1b}[1;2;3;4;5;7;8;9;31;100m"),
    ("\u{1b}[107m", "\u{1b}[1;2;3;4;5;7;8;9;31;107m"),
    ("\u{1b}[39m", "\u{1b}[1;2;3;4;5;7;8;9;44m"),
    ("\u{1b}[49m", "\u{1b}[1;2;3;4;5;7;8;9;31m"),
    ("\u{1b}[38;5;240m", "\u{1b}[1;2;3;4;5;7;8;9;38;5;240;44m"),
    ("\u{1b}[48;5;5m", "\u{1b}[1;2;3;4;5;7;8;9;31;48;5;5m"),
    (
        "\u{1b}[38;2;1;2;3m",
        "\u{1b}[1;2;3;4;5;7;8;9;38;2;1;2;3;44m",
    ),
    (
        "\u{1b}[48;2;4;5;6m",
        "\u{1b}[1;2;3;4;5;7;8;9;31;48;2;4;5;6m",
    ),
    ("\u{1b}[01;004;031m", "\u{1b}[1;2;3;4;5;7;8;9;31;44m"),
    (
        "\u{1b}[1;2;3;4;5;7;8;9;31;44m",
        "\u{1b}[1;2;3;4;5;7;8;9;31;44m",
    ),
    ("\u{1b}[0m", ""),
    ("\u{1b}[m", ""),
    ("\u{1b}[1;;4m", "\u{1b}[1;2;3;4;5;7;8;9;31;44m"),
    ("\u{1b}[38;5m", "\u{1b}[1;2;3;4;5;7;8;9;31;44m"),
    ("\u{1b}[48;2;1m", "\u{1b}[1;2;3;4;5;7;8;9;31;44m"),
    ("\u{1b}[999m", "\u{1b}[1;2;3;4;5;7;8;9;31;44m"),
    ("\u{1b}[38;9m", "\u{1b}[1;2;3;4;5;7;8;9;31;44m"),
];

#[test]
fn sgr_continuations_keep_canonical_attributes_and_color_spelling() {
    let base = "\x1b[1;2;3;4;5;7;8;9;31;44m";
    for (control, state) in CONTROLS {
        let text = format!("{base}a{control}bc");
        let lines = wrap_preserving_styles(&text, 1);
        assert_eq!(lines.len(), 3, "{control:?}");
        assert_eq!(lines[2], format!("{state}c"), "{control:?}");
        assert!(
            lines[1].starts_with(&format!("{base}{control}b")),
            "{control:?} is written out as spelled"
        );
        assert!(lines[0].starts_with(&format!("{base}a")), "{control:?}");
    }
}

/// Text, width and wrapped lines with `<O>` for the link opener and `<C>` for its close.
type LinkCase = (&'static str, usize, &'static [&'static str]);

const LINK_CASES: &[LinkCase] = &[
    ("<O>abcd", 20, &["<O>abcd<C>"]),
    ("<O>abcd", 2, &["<O>ab<C>", "<O>cd<C>"]),
    ("<O>a b c", 1, &["<O>a<C>", "<O>b<C>", "<O>c<C>"]),
    ("<O>a\n\nb", 1, &["<O>a<C>", "", "<O>b<C>"]),
    ("<O>a\n\nb", 20, &["<O>a<C>", "", "<O>b<C>"]),
    ("<O>a\x1b[0mb", 20, &["<O>a\x1b[0mb<C>"]),
    ("<O>a\x1b[0mb", 1, &["<O>a<C>", "<O>\x1b[0mb<C>"]),
    ("<O>\x1b[4mab", 20, &["<O>\x1b[4mab<C>"]),
    (
        "<O>\x1b[4mab",
        1,
        &["<O>\x1b[4ma\x1b[24m<C>", "\x1b[4m<O>b<C>"],
    ),
    ("<O>a<C>b", 20, &["<O>a<C>b"]),
    ("<O>", 1, &[""]),
    ("<O>\n<O>", 1, &["", ""]),
];

#[test]
fn hyperlinks_close_fitting_final_and_literal_lines() {
    for (opener, close) in [("\x07", "\x1b]8;;\x07"), ("\x1b\\", "\x1b]8;;\x1b\\")] {
        let open = format!("\x1b]8;id=z;https://example.com{opener}");
        let fill = |template: &str| template.replace("<O>", &open).replace("<C>", close);
        for (text, width, expected) in LINK_CASES {
            let lines = wrap_preserving_styles(&fill(text), *width);
            let expected: Vec<String> = expected.iter().map(|line| fill(line)).collect();
            assert_eq!(lines, expected, "{text:?} at {width}");
        }
    }
    let params = "\x1b]8;id=1;https://example.com/a;b\x07abc";
    assert_eq!(
        wrap_text_with_ansi(params, 1).last().map(String::as_str),
        Some("\x1b]8;id=1;https://example.com/a;b\x07c\x1b]8;;\x07")
    );
}

#[test]
fn metadata_before_dropped_leading_whitespace_moves_to_the_first_line() {
    assert_eq!(
        wrap_text_with_ansi(&format!("{MARKER} abcdef"), 2),
        [format!("{MARKER}ab"), "cd".to_owned(), "ef".to_owned()]
    );
    assert_eq!(
        wrap_text_with_ansi(&format!("{MARKER}  ab"), 2),
        [format!("{MARKER}ab")]
    );
}
