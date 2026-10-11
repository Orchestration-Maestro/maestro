//! Group behavior through package matching.

use maestro_packages::minimatch;
use std::collections::BTreeMap;

/// Replays exact tuples through the public operation.
#[cfg(test)]
fn check_group(name: &str) {
    let groups: BTreeMap<String, Vec<serde_json::Value>> =
        serde_json::from_str(include_str!("pattern_group_vectors.json")).unwrap();
    let rows = &groups[name];
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

/// Choice selects one branch, including an embedded empty branch.
#[test]
fn groups_select_exactly_one_alternative() {
    check_group("groups_select_exactly_one_alternative");
}

/// Optional groups accept zero or one branch.
#[test]
fn groups_make_one_alternative_optional() {
    check_group("groups_make_one_alternative_optional");
}

/// Repetition closes nullable branches without losing progress.
#[test]
fn groups_repeat_zero_or_more_alternatives() {
    check_group("groups_repeat_zero_or_more_alternatives");
}

/// Required repetition needs a first branch, which may itself be nullable.
#[test]
fn groups_require_one_or_more_alternatives() {
    check_group("groups_require_one_or_more_alternatives");
}

/// Exclusions include suffixes and enclosing tails.
#[test]
fn negative_groups_include_the_following_context() {
    check_group("negative_groups_include_the_following_context");
}

/// Nested alternatives preserve multiplicity and nullable branches.
#[test]
fn nested_groups_keep_repetition_and_choice() {
    check_group("nested_groups_keep_repetition_and_choice");
}

/// Initial and later iterations apply their own dot guards.
#[test]
fn group_dot_guards_follow_the_initial_alternative() {
    check_group("group_dot_guards_follow_the_initial_alternative");
}

/// Malformed delimiters and empty positive bodies retain authored fallback.
#[test]
fn unclosed_and_empty_groups_keep_literal_fallback() {
    check_group("unclosed_and_empty_groups_keep_literal_fallback");
}

/// A final empty branch does not erase prior exclusions.
#[test]
fn negative_groups_preserve_nonempty_alternatives() {
    check_group("negative_groups_preserve_nonempty_alternatives");
}

/// Escaped delimiters retain literal identity.
#[test]
fn escaped_group_separators_remain_literal() {
    check_group("escaped_group_separators_remain_literal");
}

/// Deep valid nesting remains recognizable.
#[test]
fn nested_groups_do_not_have_a_depth_cutoff() {
    check_group("nested_groups_do_not_have_a_depth_cutoff");
}

/// Grouped wildcards and classes consume scalar values.
#[test]
fn group_members_consume_unicode_scalars() {
    check_group("group_members_consume_unicode_scalars");
}
