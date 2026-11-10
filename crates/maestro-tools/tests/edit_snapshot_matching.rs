//! Original-snapshot edit matching against recorded results.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

use maestro_tools::previews::{
    Edit, FuzzyMatchResult, LineEnding, apply_edits_to_normalized_content, detect_line_ending,
    fuzzy_find_text, normalize_for_fuzzy_match, restore_line_endings, strip_bom,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

/// One authored replacement as recorded.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RecordedEdit {
    old_text: String,
    new_text: String,
}

/// One recorded call: the operation and its complete argument list.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, tag = "operation", content = "arguments")]
enum Input {
    #[serde(rename = "detectLineEnding")]
    DetectLineEnding((String,)),
    #[serde(rename = "restoreLineEndings")]
    RestoreLineEndings((String, String)),
    #[serde(rename = "stripBom")]
    StripBom((String,)),
    #[serde(rename = "normalizeForFuzzyMatch")]
    NormalizeForFuzzyMatch((String,)),
    #[serde(rename = "fuzzyFindText")]
    FuzzyFindText((String, String)),
    #[serde(rename = "applyEditsToNormalizedContent")]
    Apply((String, Vec<RecordedEdit>, String)),
}

/// One recorded row.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    input: Input,
    expected: Value,
}

/// Run the recorded operation and project its complete result.
fn observe(input: &Input) -> Value {
    match input {
        Input::DetectLineEnding((content,)) => match detect_line_ending(content) {
            LineEnding::Lf => json!("\n"),
            LineEnding::CrLf => json!("\r\n"),
        },
        Input::RestoreLineEndings((text, ending)) => {
            let ending = if ending == "\r\n" {
                LineEnding::CrLf
            } else {
                LineEnding::Lf
            };
            json!(restore_line_endings(text, ending))
        }
        Input::StripBom((content,)) => {
            let (bom, text) = strip_bom(content);
            json!({"bom": bom, "text": text})
        }
        Input::NormalizeForFuzzyMatch((text,)) => json!(normalize_for_fuzzy_match(text)),
        Input::FuzzyFindText((content, old)) => match fuzzy_find_text(content, old) {
            FuzzyMatchResult::Found {
                index,
                match_length,
                used_fuzzy_match,
                content_for_replacement,
            } => json!({
                "found": true,
                "index": index,
                "matchLength": match_length,
                "usedFuzzyMatch": used_fuzzy_match,
                "contentForReplacement": content_for_replacement,
            }),
            FuzzyMatchResult::NotFound {
                content_for_replacement,
            } => json!({
                "found": false,
                "index": -1,
                "matchLength": 0,
                "usedFuzzyMatch": false,
                "contentForReplacement": content_for_replacement,
            }),
        },
        Input::Apply((content, edits, path)) => {
            let edits: Vec<Edit> = edits
                .iter()
                .map(|edit| Edit {
                    old_text: edit.old_text.clone(),
                    new_text: edit.new_text.clone(),
                })
                .collect();
            match apply_edits_to_normalized_content(content, &edits, path) {
                Ok(done) => {
                    json!({"baseContent": done.base_content, "newContent": done.new_content})
                }
                Err(error) => json!({"error": error.message}),
            }
        }
    }
}

/// Every recorded group, keyed by test name.
fn groups() -> BTreeMap<String, Vec<Row>> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Corpus {
        tests: BTreeMap<String, Vec<Row>>,
    }
    let corpus: Corpus =
        serde_json::from_str(include_str!("fixtures/edit-matching-cases.json")).expect("fixture");
    corpus.tests
}

/// Check every row of one group against the implementation.
fn check(name: &str) {
    let rows = groups().remove(name).expect("group exists");
    assert!(!rows.is_empty());
    for (index, row) in rows.iter().enumerate() {
        assert_eq!(observe(&row.input), row.expected, "{name}[{index}]");
    }
}

#[test]
fn line_endings_follow_first_lf() {
    check("line_endings_follow_first_lf");
}

#[test]
fn line_endings_restore_only_lf() {
    check("line_endings_restore_only_lf");
}

#[test]
fn bom_removal_is_prefix_only() {
    check("bom_removal_is_prefix_only");
}

#[test]
fn fuzzy_normalization_applies_nfkc_before_line_trimming() {
    check("fuzzy_normalization_applies_nfkc_before_line_trimming");
}

#[test]
fn fuzzy_normalization_maps_punctuation_and_spaces() {
    check("fuzzy_normalization_maps_punctuation_and_spaces");
}

#[test]
fn fuzzy_trailing_space_uses_authored_whitespace_set() {
    check("fuzzy_trailing_space_uses_authored_whitespace_set");
}

#[test]
fn fuzzy_matching_prefers_exact_and_reports_byte_ranges() {
    check("fuzzy_matching_prefers_exact_and_reports_byte_ranges");
}

#[test]
fn fuzzy_matching_uses_normalized_space_or_retains_missing_input() {
    check("fuzzy_matching_uses_normalized_space_or_retains_missing_input");
}

#[test]
fn empty_normalized_needles_are_exact_only() {
    check("empty_normalized_needles_are_exact_only");
}

#[test]
fn snapshot_edits_are_disjoint_original_matches() {
    check("snapshot_edits_are_disjoint_original_matches");
}

#[test]
fn snapshot_edits_normalize_operands_not_replacement_typography() {
    check("snapshot_edits_normalize_operands_not_replacement_typography");
}

#[test]
fn snapshot_fuzzy_match_normalizes_shared_base() {
    check("snapshot_fuzzy_match_normalizes_shared_base");
}

#[test]
fn snapshot_errors_keep_validation_precedence() {
    check("snapshot_errors_keep_validation_precedence");
}

#[test]
fn snapshot_duplicates_include_fuzzy_equivalents() {
    check("snapshot_duplicates_include_fuzzy_equivalents");
}

#[test]
fn snapshot_duplicates_count_overlapping_occurrences() {
    check("snapshot_duplicates_count_overlapping_occurrences");
}

#[test]
fn snapshot_overlap_errors_keep_stable_source_order() {
    check("snapshot_overlap_errors_keep_stable_source_order");
}

#[test]
fn snapshot_no_change_compares_final_normalized_base() {
    check("snapshot_no_change_compares_final_normalized_base");
}

#[test]
fn snapshot_matching_accepts_supported_unicode_forms() {
    check("snapshot_matching_accepts_supported_unicode_forms");
}

#[test]
fn snapshot_corrected_matching_keeps_shared_base_and_error_order() {
    check("snapshot_corrected_matching_keeps_shared_base_and_error_order");
}

#[test]
fn every_recorded_group_has_a_test_and_no_query_repeats() {
    let source = include_str!("edit_snapshot_matching.rs");
    let rows = groups();
    for name in rows.keys() {
        assert!(source.contains(&format!("check(\"{name}\")")), "{name}");
    }
    let total: usize = rows.values().map(Vec::len).sum();
    assert_eq!((rows.len(), total), (19, 176));
    let raw: Value =
        serde_json::from_str(include_str!("fixtures/edit-matching-cases.json")).expect("fixture");
    let queries: BTreeSet<String> = raw["tests"]
        .as_object()
        .expect("tests")
        .values()
        .flat_map(|rows| rows.as_array().expect("rows"))
        .map(|row| row["input"].to_string())
        .collect();
    assert_eq!(queries.len(), total);
}
