//! Truncation keeps a contiguous prefix and frames the ellipsis on its own.

use maestro_tui::{TruncateOptions, truncate_to_width, visible_width};

mod fixtures {
    pub mod truncation_cases;
}
use fixtures::truncation_cases::PREFIX_CASES;

const RESET: &str = "\x1b[0m";

fn truncate(text: &str, width: usize, ellipsis: &str) -> String {
    truncate_to_width(
        text,
        width,
        TruncateOptions {
            ellipsis,
            pad: false,
        },
    )
}

fn truncate_padded(text: &str, width: usize, ellipsis: &str) -> String {
    truncate_to_width(
        text,
        width,
        TruncateOptions {
            ellipsis,
            pad: true,
        },
    )
}

#[test]
fn ellipsis_styles_are_finalized_independently() {
    for terminator in ["\x07", "\x1b\\"] {
        let open = format!("\x1b]8;;url{terminator}");
        let close = format!("\x1b]8;;{terminator}");
        let linked = format!("{open}.");
        let wide = format!("{open}\u{1f642}");
        let cases = [
            (&linked, 1, format!("{RESET}{open}.{close}{RESET}")),
            (&linked, 2, format!("a{RESET}{open}.{close}{RESET}")),
            (&linked, 4, format!("abc{RESET}{open}.{close}{RESET}")),
            (&wide, 1, String::new()),
            (&wide, 2, format!("{RESET}{open}\u{1f642}{close}{RESET}")),
            (&wide, 4, format!("ab{RESET}{open}\u{1f642}{close}{RESET}")),
        ];
        for (ellipsis, width, expected) in cases {
            assert_eq!(
                truncate("abcdefgh", width, ellipsis),
                expected,
                "{ellipsis:?} {width}"
            );
        }
    }
    for (width, expected) in [
        (1, format!("{RESET}\x1b[31m.{RESET}")),
        (2, format!("a{RESET}\x1b[31m.{RESET}")),
        (4, format!("abc{RESET}\x1b[31m.{RESET}")),
    ] {
        assert_eq!(truncate("abcdefgh", width, "\x1b[31m."), expected);
    }
    let marks = [
        ("\u{301}", format!("{RESET}\u{301}{RESET}")),
        ("\x1b[31m", String::new()),
        ("\x1b[31m\u{301}", format!("{RESET}\x1b[31m\u{301}{RESET}")),
    ];
    for (ellipsis, expected) in marks {
        assert_eq!(
            truncate("\u{754c}\u{754c}", 1, ellipsis),
            expected,
            "{ellipsis:?}"
        );
    }
}

#[test]
fn empty_ellipsis_still_resets_retained_text() {
    let text = format!("\x1b[31m{}", "hello".repeat(100));
    let truncated = truncate(&text, 10, "");
    assert_eq!(truncated, format!("\x1b[31mhellohello{RESET}"));
    assert_eq!(visible_width(&truncated), 10);
}

#[test]
fn fitting_text_ignores_oversized_ellipsis() {
    assert_eq!(truncate("\u{754c}", 2, "\u{1f642}"), "\u{754c}");
}

#[test]
fn large_unicode_prefix_fits_without_splitting() {
    let text = "\u{1f642}\u{754c}".repeat(100_000);
    let truncated = truncate(&text, 40, "\u{2026}");
    assert_eq!(
        truncated,
        format!(
            "{}\u{1f642}{RESET}\u{2026}{RESET}",
            "\u{1f642}\u{754c}".repeat(9)
        )
    );
    assert!(visible_width(&truncated) <= 40);
}

/// Text, width, ellipsis, padding and result with `<O>` for the link opener and `<C>` for its close.
type LinkCase = (&'static str, usize, &'static str, bool, &'static str);

const LINK_CASES: &[LinkCase] = &[
    ("<O>abcd", 2, "", false, "<O>ab<C>\x1b[0m"),
    ("<O>abcd", 20, "...", true, "<O>abcd<C>                "),
    ("<O>abcd", 3, "...", false, "\x1b[0m...\x1b[0m"),
    ("<O>abcde", 4, "!", false, "<O>abc<C>\x1b[0m!\x1b[0m"),
    ("<O>a b c", 2, "", false, "<O>a <C>\x1b[0m"),
    ("<O>a\x1b[0mb", 2, "", false, "<O>a\x1b[0mb<C>"),
    (
        "<O>\x1b[4mab",
        20,
        "...",
        true,
        "<O>\x1b[4mab<C>                  ",
    ),
    ("<O>a<C>b", 20, "...", false, "<O>a<C>b"),
    ("<O>", 2, "", false, ""),
    ("<O>", 20, "...", true, "                    "),
];

#[test]
fn maestro_truncation_closes_retained_links() {
    for terminator in ["\x07", "\x1b\\"] {
        let open = format!("\x1b]8;id=z;https://example.com{terminator}");
        let close = format!("\x1b]8;;{terminator}");
        let fill = |template: &str| template.replace("<O>", &open).replace("<C>", &close);
        for (text, width, ellipsis, pad, expected) in LINK_CASES {
            let options = TruncateOptions {
                ellipsis,
                pad: *pad,
            };
            let result = truncate_to_width(&fill(text), *width, options);
            assert_eq!(result, fill(expected), "{text:?} {width}");
        }
    }
}

#[test]
fn malformed_escape_prefix_finishes_with_bounded_output() {
    let text = format!("abc\x1bnot-ansi {}", "\u{1f642}".repeat(1000));
    assert!(visible_width(&truncate(&text, 20, "\u{2026}")) <= 20);
    let unterminated = "\x1b[".repeat(100_000);
    assert_eq!(visible_width(&unterminated), 100_000);
    assert!(visible_width(&truncate(&unterminated, 20, "\u{2026}")) <= 20);
}

#[test]
fn prefix_selection_stops_before_first_nonfitting_cluster() {
    assert_eq!(
        truncate_padded("\u{1f642}\t\u{754c} \x1b_abc\x07", 7, "\u{2026}"),
        format!("\u{1f642}\t{RESET}\u{2026}{RESET} ")
    );
}

#[test]
fn retained_color_resets_around_ellipsis() {
    let text = format!("\x1b[31m{}{RESET}", "hello ".repeat(1000));
    let truncated = truncate(&text, 20, "\u{2026}");
    assert_eq!(
        truncated,
        format!("\x1b[31mhello hello hello h{RESET}\u{2026}{RESET}")
    );
    assert!(visible_width(&truncated) <= 20);
}

#[test]
fn truncated_padding_reaches_requested_cells() {
    let truncated = truncate_padded(
        "\u{1f642}\u{754c}\u{1f642}\u{754c}\u{1f642}\u{754c}",
        8,
        "\u{2026}",
    );
    assert_eq!(visible_width(&truncated), 8);
    assert_eq!(
        truncated,
        format!("\u{1f642}\u{754c}\u{1f642}{RESET}\u{2026}{RESET} ")
    );
}

#[test]
fn truncation_preserves_empty_zero_width_and_prefix_cases() {
    for (text, width, ellipsis, pad, expected) in PREFIX_CASES {
        let options = TruncateOptions {
            ellipsis,
            pad: *pad,
        };
        let result = truncate_to_width(text, *width, options);
        assert_eq!(result, *expected, "{text:?} {width} {ellipsis:?} {pad}");
        if *pad && *width > 0 {
            assert_eq!(
                visible_width(&result),
                *width,
                "{text:?} {width} {ellipsis:?}"
            );
        }
        assert_ne!(result, RESET, "{text:?} {width} {ellipsis:?}");
    }
}

#[test]
fn clipped_ellipsis_keeps_content_without_cells() {
    let kept = format!("{RESET}\u{301}{RESET}");
    assert_eq!(truncate("ab", 1, "\u{301}\u{754c}"), kept);
    assert_eq!(
        truncate_padded("ab", 1, "\u{301}\u{754c}"),
        format!("{kept} ")
    );
}

#[test]
fn padding_fills_what_the_finished_result_measures() {
    for (prefix, ellipsis) in [("\u{1f1e8}", "\u{1f1e6}"), ("\u{1f44d}", "\u{1f3fd}")] {
        let joined = truncate_padded(&format!("{prefix}abcd"), 4, ellipsis);
        assert_eq!(joined, format!("{prefix}{RESET}{ellipsis}{RESET}  "));
        assert_eq!(visible_width(&joined), 4, "{prefix:?}");
    }
}
