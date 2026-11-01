//! Recorded display cases and their strict corpus validation.
#![cfg(test)]
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashSet;

/// One recorded invocation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    /// Owning public test.
    pub test: String,
    /// Distinguishable case label.
    pub id: String,
    /// Operation selected at the public boundary.
    pub op: String,
    /// Operation-specific input.
    pub input: Value,
    /// Independently recorded expectation.
    pub expected: Value,
    /// Complete macOS layout when its key names differ.
    pub expected_macos: Option<Value>,
}

/// The committed search corpus of one tool family.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    /// Theme keys, whose index plus one is each key's palette color.
    theme_keys: Vec<String>,
    /// Recorded invocations.
    cases: Vec<Case>,
}

/// The committed display recording.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Recording {
    /// Recorded invocations.
    cases: Vec<Case>,
}

/// Select a nonempty search group after validating the corpus and query identities.
pub fn search_cases(source: &str, test: &str) -> Vec<Case> {
    let corpus: Corpus = serde_json::from_str(source).unwrap();
    let mut seen = HashSet::new();
    for case in &corpus.cases {
        assert!(
            seen.insert((&case.test, &case.op, case.input.to_string())),
            "duplicate query"
        );
    }
    assert_eq!(corpus.theme_keys.len(), 7);
    let selected: Vec<_> = corpus
        .cases
        .into_iter()
        .filter(|case| case.test == test)
        .map(|mut case| {
            if cfg!(target_os = "macos") {
                case.expected = case.expected_macos.take().unwrap_or(case.expected);
            }
            case
        })
        .collect();
    assert!(!selected.is_empty(), "missing group {test}");
    selected
}

/// Return the recorded search palette values in their supplied order.
pub fn search_keys(source: &str) -> Vec<String> {
    serde_json::from_str::<Corpus>(source).unwrap().theme_keys
}

/// Every (test, operation) group the corpus may hold; a case outside them is never run.
const GROUPS: [(&str, &str); 31] = [
    (
        "read_titles_keep_argument_precedence_and_invalid_styles",
        "call",
    ),
    (
        "read_line_ranges_preserve_numeric_spelling_and_presence",
        "call",
    ),
    (
        "read_classification_obeys_skill_docs_resource_priority",
        "call",
    ),
    (
        "classified_reads_hide_only_successful_compact_results",
        "result",
    ),
    ("expanded_read_titles_return_to_authored_paths", "call"),
    ("expanded_read_results_reveal_classified_content", "result"),
    ("read_errors_bypass_compact_hiding", "result"),
    ("compact_read_ranges_precede_live_expand_hints", "call"),
    (
        "read_preview_limits_count_logical_lines_before_wrapping",
        "result",
    ),
    (
        "read_trailing_empty_rows_do_not_trim_visible_whitespace",
        "result",
    ),
    (
        "read_truncation_notices_follow_reason_and_explicit_limits",
        "result",
    ),
    (
        "read_highlighting_uses_only_selected_filename_language",
        "result",
    ),
    (
        "read_image_fallback_obeys_capability_and_visibility",
        "result",
    ),
    (
        "read_image_indicators_keep_order_and_dimension_fallbacks",
        "result",
    ),
    (
        "display_stripping_matches_complete_escape_sequences",
        "strip",
    ),
    ("shell_sanitization_keeps_only_allowed_controls", "sanitize"),
    (
        "read_output_sanitizes_before_tab_expansion_and_layout",
        "result",
    ),
    (
        "display_string_selection_distinguishes_absent_and_wrong_types",
        "str",
    ),
    (
        "display_literal_transforms_preserve_other_whitespace",
        "tabs",
    ),
    ("display_literal_transforms_preserve_other_whitespace", "cr"),
    (
        "display_paths_abbreviate_only_home_and_descendants",
        "shorten",
    ),
    (
        "key_hints_use_current_bindings_order_and_separate_styles",
        "keys",
    ),
    ("display_width_cases_use_shared_toolkit_boundaries", "width"),
    ("read_renderers_reuse_the_supplied_text_slot", "reuse"),
    (
        "display_output_handles_absence_and_orders_content_by_kind",
        "output",
    ),
    (
        "read_context_uses_current_cwd_and_documentation_path",
        "call",
    ),
    (
        "read_renderer_errors_leave_previous_text_unchanged",
        "error",
    ),
    (
        "classified_results_avoid_unused_display_dependencies",
        "early_return",
    ),
    (
        "read_highlighting_reads_the_current_published_theme",
        "result",
    ),
    (
        "read_result_uses_options_expansion_not_unrelated_flags",
        "result",
    ),
    (
        "documentation_paths_respect_windows_roots_and_separators",
        "classification_windows",
    ),
];

/// Input members every case may carry; reading one not listed here fails the test.
const MEMBERS: [&str; 12] = [
    "args",
    "context",
    "result",
    "options",
    "text",
    "value",
    "width",
    "absolutePath",
    "readme",
    "path",
    "binding",
    "slot",
];

/// Context members a case may carry.
const CONTEXT_MEMBERS: [&str; 10] = [
    "readme",
    "bindings",
    "expanded",
    "images",
    "isError",
    "showImages",
    "cwd",
    "executionStarted",
    "argsComplete",
    "themeShift",
];

/// The nonempty group of one test, after rejecting repeated queries and unknown members.
pub fn cases(test: &str) -> Vec<Case> {
    let corpus: Recording =
        serde_json::from_str(include_str!("fixtures/read_display.json")).unwrap();
    let mut queries = HashSet::new();
    for case in &corpus.cases {
        assert!(
            GROUPS.contains(&(case.test.as_str(), case.op.as_str())),
            "case {} is in no test group",
            case.id
        );
        assert!(
            queries.insert((&case.op, case.input.to_string())),
            "duplicate query {}",
            case.id
        );
        let input = case.input.as_object().unwrap();
        assert!(
            input.keys().all(|key| MEMBERS.contains(&key.as_str())),
            "unknown input member"
        );
        if let Some(context) = input.get("context") {
            let context = context.as_object().unwrap();
            assert!(
                context
                    .keys()
                    .all(|key| CONTEXT_MEMBERS.contains(&key.as_str())),
                "unknown context member"
            );
        }
    }
    let selected: Vec<_> = corpus
        .cases
        .into_iter()
        .filter(|case| case.test == test)
        .map(|mut case| {
            if cfg!(target_os = "macos") {
                case.expected = case.expected_macos.take().unwrap_or(case.expected);
            }
            case
        })
        .collect();
    assert!(!selected.is_empty(), "empty test group {test}");
    selected
}
