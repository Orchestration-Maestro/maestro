#![cfg(test)]
//! Deletion observations through the existing configured parser seam.
use maestro_markdown::{Kind, MarkedOptions, Node, parse_with_options};
use std::ops::Range;

use crate::observations;

#[test]
fn deletion_keeps_unclosed_approximations_literal() {
    observations::check(
        "deletion_keeps_unclosed_approximations_literal",
        3,
        include_str!("fixtures/deletion_scanning.json"),
    );
}

#[test]
fn deletion_preserves_caller_width_and_minimum_body() {
    observations::check(
        "deletion_preserves_caller_width_and_minimum_body",
        6,
        include_str!("fixtures/deletion_scanning.json"),
    );
}

#[test]
fn deletion_preserves_flanking_and_whitespace() {
    observations::check(
        "deletion_preserves_flanking_and_whitespace",
        6,
        include_str!("fixtures/deletion_scanning.json"),
    );
}

#[test]
fn deletion_preserves_escape_units_and_line_terminators() {
    observations::check(
        "deletion_preserves_escape_units_and_line_terminators",
        13,
        include_str!("fixtures/deletion_scanning.json"),
    );
}

#[test]
fn deletion_selects_first_raw_closer() {
    observations::check(
        "deletion_selects_first_raw_closer",
        5,
        include_str!("fixtures/deletion_scanning.json"),
    );
}

#[test]
fn deletion_respects_earlier_operands_and_recursive_labels() {
    observations::check(
        "deletion_respects_earlier_operands_and_recursive_labels",
        3,
        include_str!("fixtures/deletion_scanning.json"),
    );
}

#[test]
fn deletion_keeps_unicode_source_extents() {
    observations::check(
        "deletion_keeps_unicode_source_extents",
        2,
        include_str!("fixtures/deletion_scanning.json"),
    );
    for options in [
        MarkedOptions::Terminal,
        MarkedOptions::Browser { escape_html: false },
    ] {
        assert_strike_extent("é ~~😀e\u{301}~~ fin", options, 3..14, 5..12);
        assert_strike_extent("~~a\r\nb~~\r", options, 0..7, 2..5);
    }
}

/// Assert the selected strike's own source extent and the body its children span.
fn assert_strike_extent(
    source: &str,
    options: MarkedOptions,
    strike: Range<usize>,
    body: Range<usize>,
) {
    let document = parse_with_options(maestro_text::normalize_to_lf(source), options);
    let [
        Node {
            kind: Kind::Paragraph(children),
            ..
        },
    ] = document.nodes.as_slice()
    else {
        panic!("{source:?} is not one paragraph")
    };
    let selected = children
        .iter()
        .find(|node| matches!(node.kind, Kind::Strike(_)))
        .unwrap_or_else(|| panic!("{source:?} has no strike"));
    assert_eq!(selected.range, strike, "{source:?}");
    let Kind::Strike(inner) = &selected.kind else {
        unreachable!()
    };
    let (first, last) = (
        inner
            .first()
            .unwrap_or_else(|| panic!("{source:?} is empty")),
        inner
            .last()
            .unwrap_or_else(|| panic!("{source:?} is empty")),
    );
    assert_eq!(first.range.start..last.range.end, body, "{source:?}");
}
