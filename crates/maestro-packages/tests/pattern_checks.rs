//! Basic package pattern behavior through the public operation.
use maestro_packages::{PatternError, minimatch};
use serde::Deserialize;
use std::collections::BTreeMap;

/// A literal or a compact long-input recipe.
#[derive(Deserialize)]
#[serde(untagged)]
enum Text {
    /// Authored text.
    Literal(String),
    /// Repeated text represented without a large fixture.
    Recipe(Recipe),
}

/// Lossless construction of long inputs.
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum Recipe {
    /// Repeated scalar text.
    Repeat {
        /// Repeated unit.
        text: String,
        /// Number of units.
        count: usize,
        /// Optional preceding text.
        #[serde(default)]
        prefix: String,
    },
    /// Repeated ordered globstar sections.
    GlobstarPattern {
        /// Section count.
        count: usize,
        /// Text after the final globstar.
        #[serde(default)]
        suffix: String,
    },
    /// Input for repeated globstar sections.
    GlobstarInput {
        /// Component count.
        count: usize,
        /// Terminal text.
        #[serde(default)]
        suffix: String,
    },
}

impl Text {
    /// Materializes the exact recorded query operand.
    fn materialize(self) -> String {
        match self {
            Self::Literal(text) => text,
            Self::Recipe(Recipe::Repeat {
                text,
                count,
                prefix,
            }) => {
                format!("{prefix}{}", text.repeat(count))
            }
            Self::Recipe(Recipe::GlobstarPattern { count, suffix }) => {
                format!("{}**{suffix}", "**/a/".repeat(count))
            }
            Self::Recipe(Recipe::GlobstarInput { count, suffix }) => {
                format!("{}{suffix}", "a/".repeat(count))
            }
        }
    }
}

/// Recorded boolean or admitted error.
#[derive(Deserialize)]
#[serde(untagged)]
enum Expected {
    /// Match result.
    Match(bool),
    /// Validation failure.
    Error(Error),
}

/// Exact reachable error text.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Error {
    /// Display message.
    error: String,
}

/// One unique public-operation query.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Pattern operand.
    pattern: Text,
    /// Path operand.
    input: Text,
    /// Independently recorded or corrected result.
    expected: Expected,
}

/// Selects a test's queries and checks every retained field.
#[cfg(test)]
fn check(name: &str) {
    let mut groups: BTreeMap<String, Vec<Case>> =
        serde_json::from_str(include_str!("pattern_vectors.json")).unwrap();
    let cases = groups.remove(name).unwrap();
    assert!(!cases.is_empty());
    for (index, case) in cases.into_iter().enumerate() {
        let result = minimatch(&case.input.materialize(), &case.pattern.materialize());
        match case.expected {
            Expected::Match(expected) => assert_eq!(result, Ok(expected), "case {index}"),
            Expected::Error(expected) => {
                assert_eq!(result, Err(PatternError::TooLong), "case {index}");
                assert_eq!(
                    result.unwrap_err().to_string(),
                    expected.error,
                    "case {index}"
                );
            }
        }
    }
}

/// Exercises literal matching is case sensitive.
#[test]
fn literal_matching_is_case_sensitive() {
    check("literal_matching_is_case_sensitive");
}

/// Exercises wildcards stay inside nonempty components.
#[test]
fn wildcards_stay_inside_nonempty_components() {
    check("wildcards_stay_inside_nonempty_components");
}

/// Exercises question matches one unicode scalar.
#[test]
fn question_matches_one_unicode_scalar() {
    check("question_matches_one_unicode_scalar");
}

/// Exercises globstars span only allowed components.
#[test]
fn globstars_span_only_allowed_components() {
    check("globstars_span_only_allowed_components");
}

/// Exercises explicit dots and traversal are distinct.
#[test]
fn explicit_dots_and_traversal_are_distinct() {
    check("explicit_dots_and_traversal_are_distinct");
}

/// Exercises brace alternatives and sequences match end to end.
#[test]
fn brace_alternatives_and_sequences_match_end_to_end() {
    check("brace_alternatives_and_sequences_match_end_to_end");
}

/// Exercises backslash escapes preserve literal intent.
#[test]
fn backslash_escapes_preserve_literal_intent() {
    check("backslash_escapes_preserve_literal_intent");
}

/// Exercises unclosed classes remain literal without hiding wildcards.
#[test]
fn unclosed_classes_remain_literal_without_hiding_wildcards() {
    check("unclosed_classes_remain_literal_without_hiding_wildcards");
}

/// Exercises comments precede negation and empty matching.
#[test]
fn comments_precede_negation_and_empty_matching() {
    check("comments_precede_negation_and_empty_matching");
}

/// Exercises pattern parents reduce without normalizing input.
#[test]
fn pattern_parents_reduce_without_normalizing_input() {
    check("pattern_parents_reduce_without_normalizing_input");
}

/// Exercises trailing separators keep directory semantics.
#[test]
fn trailing_separators_keep_directory_semantics() {
    check("trailing_separators_keep_directory_semantics");
}

/// Exercises stars backtrack without crossing separators.
#[test]
fn stars_backtrack_without_crossing_separators() {
    check("stars_backtrack_without_crossing_separators");
}

/// Exercises globstar sections keep head tail and order.
#[test]
fn globstar_sections_keep_head_tail_and_order() {
    check("globstar_sections_keep_head_tail_and_order");
}

/// Exercises validation counts scalars before shortcuts.
#[test]
fn validation_counts_scalars_before_shortcuts() {
    check("validation_counts_scalars_before_shortcuts");
}

/// Exercises globstars do not have a recursion false negative.
#[test]
fn globstars_do_not_have_a_recursion_false_negative() {
    check("globstars_do_not_have_a_recursion_false_negative");
}

/// Exercises brace default cap selects the earlier alternatives.
#[test]
fn brace_default_cap_selects_the_earlier_alternatives() {
    check("brace_default_cap_selects_the_earlier_alternatives");
}
