//! Whitespace, lexical order and markup escaping against recorded results.
#![cfg(test)]
use maestro_text::{
    WHITESPACE, is_whitespace_scalar, markup::EscapeMode, markup::escape, markup::escape_into,
    utf16_order,
};
use serde::Deserialize;
use std::cmp::Ordering;

/// One recorded text composition.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Classification {
    input: String,
    any: bool,
    all: bool,
    trim: String,
    start: String,
    end: String,
}

/// One recorded escape result per mode.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Markup {
    input: String,
    xml: String,
    html: String,
    #[serde(rename = "htmlHex")]
    html_hex: String,
}

/// One recorded comparison.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Lexical {
    left: String,
    right: String,
    ordering: i8,
}

/// The recorded sort of a name list.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LexicalSort {
    input: Vec<String>,
    expected: Vec<String>,
}

/// All recorded text rules.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Rules {
    accepted: Vec<u32>,
    classification: Vec<Classification>,
    markup: Vec<Markup>,
    lexical: Vec<Lexical>,
    #[serde(rename = "lexicalSort")]
    lexical_sort: LexicalSort,
}

/// Load the recorded rules.
fn rules() -> Rules {
    serde_json::from_str(include_str!("fixtures/text_rules.json")).unwrap()
}

#[test]
fn scalar_whitespace_matches_recorded_membership() {
    let rules = rules();
    let mut owned: Vec<u32> = WHITESPACE.chars().map(u32::from).collect();
    owned.sort_unstable();
    assert_eq!(owned, rules.accepted);
    for value in (0..=char::MAX as u32).filter_map(char::from_u32) {
        assert_eq!(
            is_whitespace_scalar(value),
            rules.accepted.contains(&u32::from(value)),
            "U+{:04X}",
            u32::from(value)
        );
    }
}

#[test]
fn whitespace_composition_keeps_empty_and_mixed_text() {
    for case in rules().classification {
        let text = case.input.as_str();
        assert_eq!(text.chars().any(is_whitespace_scalar), case.any, "{text:?}");
        assert_eq!(text.chars().all(is_whitespace_scalar), case.all, "{text:?}");
        assert_eq!(text.trim_matches(is_whitespace_scalar), case.trim);
        assert_eq!(text.trim_start_matches(is_whitespace_scalar), case.start);
        assert_eq!(text.trim_end_matches(is_whitespace_scalar), case.end);
    }
}

#[test]
fn lexical_order_keeps_utf16_sequence_and_duplicates() {
    let rules = rules();
    for case in &rules.lexical {
        let expected = match case.ordering {
            -1 => Ordering::Less,
            0 => Ordering::Equal,
            _ => Ordering::Greater,
        };
        assert_eq!(
            utf16_order(&case.left, &case.right),
            expected,
            "{case_left:?}",
            case_left = case.left
        );
    }
    let mut names = rules.lexical_sort.input;
    names.sort_by(|a, b| utf16_order(a, b));
    assert_eq!(names, rules.lexical_sort.expected);
}

#[test]
fn markup_modes_escape_once_and_append() {
    for case in rules().markup {
        for (mode, expected) in [
            (EscapeMode::Xml, &case.xml),
            (EscapeMode::HtmlDecimal, &case.html),
            (EscapeMode::HtmlHex, &case.html_hex),
        ] {
            assert_eq!(&escape(&case.input, mode), expected, "{mode:?}");
            let mut out = String::from("<sentinel>");
            escape_into(&case.input, &mut out, mode);
            assert_eq!(out, format!("<sentinel>{expected}"), "{mode:?}");
        }
    }
}
