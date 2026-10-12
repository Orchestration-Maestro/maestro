//! Case-insensitive standalone matching through the public operation.
#![cfg(test)]
use maestro_pattern::{PatternError, minimatch, minimatch_with_nocase};
use std::collections::BTreeMap;

/// One recorded observation under an explicit platform and case mode.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    /// Platform whose framing produced the expectation.
    platform: String,
    /// Whether case-insensitive matching was selected.
    nocase: bool,
    /// Authored pattern.
    pattern: String,
    /// Operand path.
    input: String,
    /// Expected match result.
    expected: bool,
}

/// Replays the host-platform rows of one named group.
fn replay(group: &str) {
    let groups: BTreeMap<String, Vec<Row>> =
        serde_json::from_str(include_str!("fixtures/nocase_vectors.json")).unwrap();
    let host = if cfg!(windows) { "win32" } else { "linux" };
    let rows = &groups[group];
    let mut replayed = 0;
    for (index, row) in rows.iter().filter(|row| row.platform == host).enumerate() {
        let result = minimatch_with_nocase(&row.input, &row.pattern, row.nocase);
        assert_eq!(result, Ok(row.expected), "{group} row {index}");
        if !row.nocase {
            assert_eq!(
                minimatch(&row.input, &row.pattern),
                Ok(row.expected),
                "{group} default row {index}"
            );
        }
        replayed += 1;
    }
    assert!(replayed > 0, "{group} replayed no rows");
}

#[test]
fn nocase_matches_literals_without_rewriting_operands() {
    replay("nocase_matches_literals_without_rewriting_operands");
}

#[test]
fn nocase_keeps_ordinary_and_unicode_case_classes_distinct() {
    replay("nocase_keeps_ordinary_and_unicode_case_classes_distinct");
}

#[test]
fn nocase_suffixes_keep_lowercase_expansion_and_cardinality() {
    replay("nocase_suffixes_keep_lowercase_expansion_and_cardinality");
}

#[test]
fn nocase_preserves_escapes_and_class_syntax() {
    replay("nocase_preserves_escapes_and_class_syntax");
}

#[test]
fn nocase_keeps_slash_dot_and_globstar_rules() {
    replay("nocase_keeps_slash_dot_and_globstar_rules");
}

#[test]
fn nocase_keeps_comment_negation_and_empty_order() {
    replay("nocase_keeps_comment_negation_and_empty_order");
}

#[test]
fn nocase_expands_braces_before_component_matching() {
    replay("nocase_expands_braces_before_component_matching");
}

#[test]
fn nocase_folds_nested_groups_and_negative_continuations() {
    replay("nocase_folds_nested_groups_and_negative_continuations");
}

#[test]
fn nocase_reuses_existing_scalar_and_group_corrections() {
    replay("nocase_reuses_existing_scalar_and_group_corrections");
}

#[test]
fn nocase_preserves_windows_root_policy() {
    replay("nocase_preserves_windows_root_policy");
}

#[test]
fn nocase_validates_scalars_before_shortcuts() {
    let admitted = "A".repeat(65_536);
    let rejected = "A".repeat(65_537);
    let astral_admitted = "😀".repeat(65_536);
    let astral_rejected = "😀".repeat(65_537);
    for nocase in [false, true] {
        assert_eq!(minimatch_with_nocase("b", &admitted, nocase), Ok(false));
        assert_eq!(
            minimatch_with_nocase("b", &astral_admitted, nocase),
            Ok(false)
        );
        for pattern in [
            rejected.clone(),
            format!("#{admitted}"),
            astral_rejected.clone(),
        ] {
            let error = minimatch_with_nocase("b", &pattern, nocase).unwrap_err();
            assert_eq!(error, PatternError::TooLong);
            assert_eq!(error.to_string(), "pattern is too long");
        }
    }
}
