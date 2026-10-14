//! Literal terminal conversion against recorded fixtures.
#![cfg(test)]
use maestro_export::{ansi_lines_to_html, ansi_to_html};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    test: String,
    input: Value,
    operation: String,
    expected: String,
}
fn check(group: &str) {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("fixtures/ansi_conversion.json")).unwrap();
    let mut count = 0;
    for case in cases.into_iter().filter(|case| case.test == group) {
        let actual = match case.operation.as_str() {
            "text" => ansi_to_html(case.input.as_str().unwrap()),
            "lines" => {
                ansi_lines_to_html(&serde_json::from_value::<Vec<String>>(case.input).unwrap())
            }
            _ => panic!("unknown operation"),
        };
        assert_eq!(actual, case.expected, "{}", case.id);
        count += 1;
    }
    assert!(count > 0);
}
#[test]
fn two_lines_have_no_source_separator() {
    check("two_lines_have_no_source_separator");
}
#[test]
fn literal_markup_is_escaped_once() {
    check("literal_markup_is_escaped_once");
}
#[test]
fn standard_and_bright_colors_share_the_palette() {
    check("standard_and_bright_colors_share_the_palette");
}
#[test]
fn every_extended_palette_index_is_expanded() {
    check("every_extended_palette_index_is_expanded");
}
#[test]
fn rgb_components_keep_decimal_css() {
    check("rgb_components_keep_decimal_css");
}
#[test]
fn styles_accumulate_and_reset_independently() {
    check("styles_accumulate_and_reset_independently");
}
#[test]
fn empty_text_and_lines_have_distinct_wrappers() {
    check("empty_text_and_lines_have_distinct_wrappers");
}
#[test]
fn out_of_palette_indices_keep_authored_expansion() {
    check("out_of_palette_indices_keep_authored_expansion");
}
#[test]
fn style_properties_have_fixed_order() {
    check("style_properties_have_fixed_order");
}
#[test]
fn sgr_boundaries_keep_empty_spans_and_parameter_defaults() {
    check("sgr_boundaries_keep_empty_spans_and_parameter_defaults");
}
#[test]
fn incomplete_colors_leave_remaining_codes_active() {
    check("incomplete_colors_leave_remaining_codes_active");
}
#[test]
fn only_ascii_digit_semicolon_sgr_is_consumed() {
    check("only_ascii_digit_semicolon_sgr_is_consumed");
}
#[test]
fn line_conversion_preserves_spacing_and_resets_style() {
    check("line_conversion_preserves_spacing_and_resets_style");
}
