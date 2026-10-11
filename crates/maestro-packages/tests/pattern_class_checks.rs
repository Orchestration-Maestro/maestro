//! Character-class behavior through package matching.

use maestro_packages::minimatch;
use std::collections::BTreeMap;

/// Replays the group's recorded public-operation results.
#[cfg(test)]
fn check_group(name: &str) {
    let groups: BTreeMap<String, Vec<serde_json::Value>> =
        serde_json::from_str(include_str!("pattern_class_vectors.json")).unwrap();
    let rows = groups.get(name).unwrap();
    assert!(!rows.is_empty());
    for (index, row) in rows.iter().enumerate() {
        let (pattern, input, expected): (String, String, bool) =
            serde_json::from_value(row.clone()).unwrap();
        assert_eq!(
            minimatch(&input, &pattern),
            Ok(expected),
            "{name} case {index}"
        );
    }
}

/// Inclusive ranges preserve surviving members and invalid endpoint selection.
#[test]
fn classes_keep_ranges_and_invalid_range_selection() {
    check_group("classes_keep_ranges_and_invalid_range_selection");
}

/// Class negation retains literal punctuation and decoded singleton identity.
#[test]
fn classes_negate_members_without_losing_literals() {
    check_group("classes_negate_members_without_losing_literals");
}

/// Named membership follows Unicode categories rather than broader helpers.
#[test]
fn named_classes_use_unicode_categories() {
    check_group("named_classes_use_unicode_categories");
}

/// Printable membership excludes Other characters, not printable text.
#[test]
fn printable_class_does_not_invert_control_characters() {
    check_group("printable_class_does_not_invert_control_characters");
}

/// Mixed members compose with path framing and consume scalar text.
#[test]
fn combined_classes_keep_positive_and_negative_members() {
    check_group("combined_classes_keep_positive_and_negative_members");
}

/// Unclosed openers leave following tokens active except for named range endpoints.
#[test]
fn unclosed_classes_preserve_following_tokens() {
    check_group("unclosed_classes_preserve_following_tokens");
}

/// Outer negation complements the complete member union once.
#[test]
fn negated_class_unions_complement_the_complete_set() {
    check_group("negated_class_unions_complement_the_complete_set");
}

/// Magic member unions cannot bypass dot and traversal protection.
#[test]
fn magic_class_unions_keep_dot_and_traversal_guards() {
    check_group("magic_class_unions_keep_dot_and_traversal_guards");
}
