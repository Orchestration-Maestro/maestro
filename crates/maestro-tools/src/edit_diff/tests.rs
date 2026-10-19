//! Recorded line sequences and numbered output.
use super::generate_diff_string;
use crate::diff::text_diff::{Change, diff_lines};
use serde::Deserialize;
use std::collections::{BTreeMap, HashSet};

/// A closed corpus grouped by its owning test.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    tests: BTreeMap<String, Vec<Case>>,
}
/// One recorded query and its complete output.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Case {
    old: String,
    new: String,
    context: Option<usize>,
    changes: Vec<(String, String)>,
    diff: String,
    first_changed_line: Option<usize>,
}
/// Assert one group's ordered runs and public text.
fn check(name: &str) {
    let corpus: Corpus =
        serde_json::from_str(include_str!("../../tests/fixtures/edit-line-diffs.json")).unwrap();
    assert_eq!(
        corpus.tests.keys().map(String::as_str).collect::<Vec<_>>(),
        vec![
            "change_runs_keep_repeated_line_alignment",
            "context_lines_keep_old_numbers_after_insertions",
            "explicit_context_changes_only_unchanged_rows",
            "generated_edit_scripts_keep_line_alignment",
            "identical_inputs_have_no_numbered_diff",
            "leading_context_keeps_nearest_lines",
            "line_tokens_preserve_separators_and_visible_scalars",
            "middle_context_obeys_double_context_boundary",
            "number_columns_use_both_complete_texts",
            "recorded_pairs_keep_numbered_text",
            "terminal_newline_changes_remain_visible",
            "trailing_context_keeps_nearest_lines"
        ]
    );
    let mut queries = HashSet::new();
    for cases in corpus.tests.values() {
        assert!(!cases.is_empty());
        for case in cases {
            assert!(queries.insert((&case.old, &case.new, case.context)));
        }
    }
    let cases = corpus.tests.get(name).unwrap();
    for (index, case) in cases.iter().enumerate() {
        let changes: Vec<_> = diff_lines(&case.old, &case.new)
            .into_iter()
            .map(|change| match change {
                Change::Added(value) => ("added", value),
                Change::Removed(value) => ("removed", value),
                Change::Unchanged(value) => ("unchanged", value),
            })
            .collect();
        let expected: Vec<_> = case
            .changes
            .iter()
            .map(|(kind, value)| (kind.as_str(), value.as_str()))
            .collect();
        assert_eq!(changes, expected, "{name} case {index}: ordered runs");
        let result = generate_diff_string(&case.old, &case.new, case.context);
        assert_eq!(result.diff, case.diff, "{name} case {index}: numbered text");
        assert_eq!(
            result.first_changed_line, case.first_changed_line,
            "{name} case {index}: first line"
        );
    }
}
#[test]
fn recorded_pairs_keep_numbered_text() {
    check("recorded_pairs_keep_numbered_text");
}

#[test]
fn line_tokens_preserve_separators_and_visible_scalars() {
    check("line_tokens_preserve_separators_and_visible_scalars");
}

#[test]
fn change_runs_keep_repeated_line_alignment() {
    check("change_runs_keep_repeated_line_alignment");
}

#[test]
fn leading_context_keeps_nearest_lines() {
    check("leading_context_keeps_nearest_lines");
}

#[test]
fn middle_context_obeys_double_context_boundary() {
    check("middle_context_obeys_double_context_boundary");
}

#[test]
fn trailing_context_keeps_nearest_lines() {
    check("trailing_context_keeps_nearest_lines");
}

#[test]
fn explicit_context_changes_only_unchanged_rows() {
    check("explicit_context_changes_only_unchanged_rows");
}

#[test]
fn number_columns_use_both_complete_texts() {
    check("number_columns_use_both_complete_texts");
}

#[test]
fn context_lines_keep_old_numbers_after_insertions() {
    check("context_lines_keep_old_numbers_after_insertions");
}

#[test]
fn identical_inputs_have_no_numbered_diff() {
    check("identical_inputs_have_no_numbered_diff");
}

#[test]
fn terminal_newline_changes_remain_visible() {
    check("terminal_newline_changes_remain_visible");
}

#[test]
fn generated_edit_scripts_keep_line_alignment() {
    check("generated_edit_scripts_keep_line_alignment");
}
