//! Greedy wrapping that keeps style and hyperlink state across line breaks.

use maestro_tui::{visible_width, wrap_text_with_ansi};

use crate::fixtures;
use fixtures::terminal::{replay, replay_lines};

const UNDERLINE_ON: &str = "\x1b[4m";
const UNDERLINE_OFF: &str = "\x1b[24m";
const RESET: &str = "\x1b[0m";
const RED: &str = "\x1b[31m";
const MARKER: &str = "\x1b_maestro:c\x07";
const PROMPT: &str = "\x1b]133;A\x07";

/// An input whose escapes sit where a row breaks, the width it wraps at, the rows it wraps into
/// and the rows `Text` pads them to.
pub(crate) struct BoundaryCase {
    /// Text to wrap.
    pub text: String,
    /// Width to wrap at.
    pub width: usize,
    /// Rows the wrapper returns.
    pub wrapped: Vec<String>,
    /// Rows `Text` renders without padding or background.
    pub rendered: Vec<String>,
}

/// Escapes written before an overflowing grapheme, after a painted wide blank or between words:
/// a background, inverse or underline reset with a marker before or after it, two metadata
/// escapes, and a closed hyperlink.
pub(crate) fn boundary_cases() -> Vec<BoundaryCase> {
    let case = |row: String, width: usize, padding: &str| BoundaryCase {
        text: format!("{row}\u{754c}"),
        width,
        rendered: vec![format!("{row} "), format!("\u{754c}{padding}")],
        wrapped: vec![row, "\u{754c}".to_owned()],
    };
    let mut cases = Vec::new();
    for (on, off) in [
        ("\x1b[44m", "\x1b[49m"),
        ("\x1b[7m", "\x1b[27m"),
        ("\x1b[4m", "\x1b[24m"),
    ] {
        for (before, after) in [
            ("", ""),
            (MARKER, ""),
            ("", MARKER),
            (PROMPT, ""),
            ("", PROMPT),
        ] {
            cases.push(case(
                format!("ab{on}\u{3000}{before}{off}{after}"),
                5,
                "   ",
            ));
        }
    }
    cases.push(case(format!("ab{PROMPT}{MARKER}"), 3, " "));
    cases.push(case(
        "\x1b]8;;https://example.com\x07ab\x1b]8;;\x07".to_owned(),
        3,
        " ",
    ));
    cases
}

/// Breaks after which a row would hold only the style it inherits: a background before a long
/// word, and a foreground blank whose prompt or cursor marker ends the line.
pub(crate) fn inherited_style_cases() -> Vec<BoundaryCase> {
    let case = |text: String, wrapped: Vec<String>, rendered: Vec<String>| BoundaryCase {
        text,
        width: 2,
        wrapped,
        rendered,
    };
    let mut cases = Vec::new();
    for marker in ["", MARKER] {
        let wrapped = vec![
            "\x1b[44mab".to_owned(),
            format!("\x1b[44m{marker}cd"),
            "\x1b[44me".to_owned(),
        ];
        let mut rendered = wrapped.clone();
        rendered[2].push(' ');
        cases.push(case(format!("\x1b[44mab {marker}cde"), wrapped, rendered));
    }
    for marker in [PROMPT, "\x1b]133;B\x07", MARKER] {
        let row = vec![format!("ab{marker}")];
        cases.push(case(format!("ab{RED} {marker}"), row.clone(), row));
        let row = vec![format!("{RED}ab{marker}")];
        cases.push(case(format!("{RED}ab {marker}"), row.clone(), row));
    }
    cases
}

/// Asserts the rows each case wraps into, naming every input that differs.
fn assert_wraps(cases: &[BoundaryCase]) {
    let actual: Vec<_> = cases
        .iter()
        .map(|case| {
            (
                case.text.as_str(),
                wrap_text_with_ansi(&case.text, case.width),
            )
        })
        .collect();
    let expected: Vec<_> = cases
        .iter()
        .map(|case| (case.text.as_str(), case.wrapped.clone()))
        .collect();
    assert_eq!(actual, expected);
}

#[test]
fn rows_never_hold_inherited_style_alone() {
    assert_wraps(&inherited_style_cases());
}

#[test]
fn long_word_escapes_precede_the_overflowing_grapheme() {
    assert_wraps(&boundary_cases());
}

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

/// An escape cannot span a literal newline: the lines are split first, so an escape whose
/// terminator sits on a later line stays text on its own line.
#[test]
fn escapes_do_not_cross_literal_newlines() {
    let cases: &[(&str, &[&str])] = &[
        ("\x1b[\nm", &["\x1b[", "m"]),
        ("\x1b]8;;u\ntext\x07", &["\x1b]8;;u", "text\x07"]),
        ("\x1b_x\ny\x07", &["\x1b_x", "y\x07"]),
        ("\x1b[31m\x1b[\nm", &["\x1b[31m\x1b[", "\x1b[31mm"]),
    ];
    for (text, expected) in cases {
        assert_eq!(wrap_text_with_ansi(text, 80), *expected, "{text:?}");
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
    ("\u{1b}[38;5;005m", "\u{1b}[1;2;3;4;5;7;8;9;38;5;005;44m"),
    (
        "\u{1b}[48;2;001;002;003m",
        "\u{1b}[1;2;3;4;5;7;8;9;31;48;2;001;002;003m",
    ),
    ("\u{1b}[1;0;31m", "\u{1b}[31m"),
    ("\u{1b}[4;00;1m", "\u{1b}[1m"),
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
            lines[1].starts_with(&format!("{state}b")),
            "{control:?} reopens the state"
        );
        assert!(
            lines[0].starts_with(&format!("{base}a{control}")),
            "{control:?} is written out as spelled before the break"
        );
    }
}

/// A CSI that ends in `m` sets the style from the parameters between its last `ESC [` and
/// that `m`, whatever text precedes them inside the sequence.
#[test]
fn style_follows_the_sgr_nested_in_a_malformed_csi() {
    let cases: &[(&str, &[&str])] = &[
        (
            "\x1b[31ma\x1b[bad\x1b[39mbc",
            &["\x1b[31ma\x1b[bad\x1b[39m", "b", "c"],
        ),
        (
            "\x1b[1;2ma\x1b[1x\x1b[22mbc",
            &["\x1b[1;2ma\x1b[1x\x1b[22m", "b", "c"],
        ),
    ];
    for (text, expected) in cases {
        assert_eq!(wrap_text_with_ansi(text, 1), *expected, "{text:?}");
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
    ("<O>a\x1b[0mb", 1, &["<O>a\x1b[0m<C>", "<O>b<C>"]),
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

/// Inverse, underline and background blanks paint a cell, so a wrapped row keeps them and the
/// plain whitespace inside the same run instead of dropping it as trailing whitespace.
#[test]
fn styled_whitespace_survives_soft_wrapping() {
    let cases: &[(&str, usize, &[&str])] = &[
        (
            "ab \u{1b}[7m \u{1b}[27m cd",
            3,
            &["ab", " \u{1b}[7m \u{1b}[27m", "cd"],
        ),
        (
            "ab \u{1b}[7m \u{1b}[27m cd",
            2,
            &["ab", " \u{1b}[7m \u{1b}[27m", "cd"],
        ),
        (
            "ab \u{1b}[7m \u{1b}[27m cd",
            4,
            &["ab", " \u{1b}[7m \u{1b}[27m", "cd"],
        ),
        (
            "ab \u{1b}[7m \u{1b}[27m cd",
            8,
            &["ab \u{1b}[7m \u{1b}[27m cd"],
        ),
        ("\u{1b}[7m \u{1b}[27m", 3, &["\u{1b}[7m \u{1b}[27m"]),
        (
            "ab\n\u{1b}[7m \u{1b}[27m\ncd",
            1,
            &["a", "b", "\u{1b}[7m \u{1b}[27m", "c", "d"],
        ),
        (
            "a\u{1b}[7m \u{1b}[7m \u{1b}[27m\u{1b}[27mb",
            2,
            &["a", "\u{1b}[7m \u{1b}[7m", "\u{1b}[7m\u{1b}[27m\u{1b}[27mb"],
        ),
        (
            "ab \u{1b}[7m \u{1b}[27m",
            3,
            &["ab", " \u{1b}[7m \u{1b}[27m"],
        ),
        ("- \u{1b}[4m \u{1b}[24m", 1, &["-", "\u{1b}[4m \u{1b}[24m"]),
        (
            "- \u{1b}[44m \u{1b}[49m",
            1,
            &["-", " \u{1b}[44m", "\u{1b}[44m \u{1b}[49m"],
        ),
    ];
    for (text, width, expected) in cases {
        assert_eq!(
            wrap_text_with_ansi(text, *width),
            *expected,
            "{text:?} at {width}"
        );
    }
}

/// Whitespace that paints nothing keeps the existing correction: escape-only pieces and
/// plain, foreground-only, bold-only and hidden-only blanks do not add rows.
#[test]
fn styled_whitespace_checks_do_not_keep_invisible_overflow() {
    let cases: &[(&str, usize, &[&str])] = &[
        (
            "ab\u{1b}[31m \u{1b}[39mcd",
            2,
            &["ab", "\u{1b}[31m\u{1b}[39mcd"],
        ),
        (
            "ab\u{1b}[1m \u{1b}[22mcd",
            2,
            &["ab", "\u{1b}[1m\u{1b}[22mcd"],
        ),
        (
            "ab\u{1b}[8m \u{1b}[28mcd",
            2,
            &["ab", "\u{1b}[8m\u{1b}[28mcd"],
        ),
        ("ab \u{1b}[7m\u{1b}[27m cd", 3, &["ab", "cd"]),
        ("ab   ", 2, &["ab"]),
        ("  ", 1, &[""]),
        ("a\n\nb", 1, &["a", "", "b"]),
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
fn consecutive_painted_continuations_keep_their_rows() {
    let mut actual_rows = Vec::new();
    let mut expected_rows = Vec::new();
    for (on, off, middle, prefix) in [
        ("\x1b[44m", "\x1b[49m", "\x1b[44m", " \x1b[44m"),
        ("\x1b[7m", "\x1b[27m", "\x1b[7m", " \x1b[7m"),
        ("\x1b[4m", "\x1b[24m", "\x1b[4m \x1b[24m", ""),
    ] {
        let mut expected = vec!["-".to_owned()];
        if !prefix.is_empty() {
            expected.push(prefix.to_owned());
        }
        expected.extend([middle.to_owned(), middle.to_owned(), format!("{on} {off}")]);
        actual_rows.push(wrap_text_with_ansi(&format!("- {on}   {off}"), 1));
        expected_rows.push(expected);
    }
    assert_eq!(actual_rows, expected_rows);
}

#[test]
fn filtered_authored_lines_keep_a_row_and_metadata() {
    let markers = ["", "\x1b]133;A\x07", MARKER];
    let actual: Vec<_> = markers
        .iter()
        .map(|marker| wrap_text_with_ansi(&format!("a\n\x1b[31m{marker}  \x1b[39m\nb"), 1))
        .collect();
    let expected: Vec<_> = markers
        .iter()
        .map(|marker| vec!["a", marker, "b"])
        .collect();
    assert_eq!(actual, expected);
}

#[test]
fn metadata_on_discarded_continuations_is_emitted_once() {
    for marker in [MARKER, "\x1b]133;A\x07"] {
        assert_eq!(
            (
                wrap_text_with_ansi(&format!("- \x1b[4m{marker} \x1b[24m"), 1),
                wrap_text_with_ansi(&format!("\t{marker}a"), 1)
            ),
            (
                vec!["-".to_owned(), format!("\x1b[4m{marker} \x1b[24m")],
                vec![format!("{marker}a")]
            )
        );
    }
}
