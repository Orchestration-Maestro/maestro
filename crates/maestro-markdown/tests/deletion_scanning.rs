#![cfg(test)]
//! Deletion observations through the existing configured parser seam.
#[path = "grammar_rules/observations.rs"]
mod observations;

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
}
