//! Visual tail selection through the public output helper.
#![cfg(test)]
use super::edit_render_support::Case;
use maestro_tools::truncate_to_visual_lines;
use serde::Deserialize;

/// Strict corpus of selector queries.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    /// Independent queries.
    cases: Vec<Case>,
}
/// Selector operands.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Input {
    /// Authored text.
    text: String,
    /// Maximum retained rows.
    max_visual_lines: usize,
    /// Viewport columns.
    width: usize,
    /// Optional horizontal padding.
    padding_x: Option<usize>,
}
/// Check exact rows and hidden count for one complete group.
fn check(test: &str) {
    let corpus: Corpus =
        serde_json::from_str(include_str!("fixtures/visual-tail-cases.json")).unwrap();
    let mut seen = std::collections::HashSet::new();
    let mut count = 0;
    for case in corpus.cases {
        assert!(
            [
                "visual_tail_wraps_before_selecting_rows",
                "visual_tail_handles_boundaries"
            ]
            .contains(&case.test.as_str())
        );
        assert_eq!(case.op, "visual");
        assert!(seen.insert(case.input.to_string()));
        let input: Input = serde_json::from_value(case.input).unwrap();
        if case.test != test {
            continue;
        }
        let result = truncate_to_visual_lines(
            &input.text,
            input.max_visual_lines,
            input.width,
            input.padding_x,
        );
        assert_eq!(
            serde_json::json!({"visualLines":result.visual_lines,"skippedCount":result.skipped_count}),
            case.expected
        );
        count += 1;
    }
    assert!(count > 0);
}
#[test]
fn visual_tail_wraps_before_selecting_rows() {
    check("visual_tail_wraps_before_selecting_rows");
}

#[test]
fn visual_tail_handles_boundaries() {
    check("visual_tail_handles_boundaries");
}
