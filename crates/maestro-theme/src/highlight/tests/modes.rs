//! Focused native recognition behaviors.

#[test]
fn mode_search_uses_earliest_match_then_rule_order() {
    super::fixtures::witnesses(&["order", "begin_end_illegal"]);
}

#[test]
fn ignored_begins_resume_without_losing_prior_rules() {
    super::fixtures::witnesses(&["before", "lookahead"]);
    super::fixtures::inputs(&["dot-before", "dot-after", "resume-followup"]);
}

#[test]
fn mode_boundaries_keep_skip_exclude_and_return_policies() {
    super::fixtures::witnesses(&[
        "included",
        "excluded",
        "excluded_begin",
        "excluded_end",
        "returned",
        "skip",
    ]);
}

#[test]
fn parent_end_and_starts_keep_buffer_ownership() {
    super::fixtures::witnesses(&["parent", "reused", "starts"]);
}

#[test]
fn scope_buffers_score_only_unclassified_captures() {
    super::fixtures::witnesses(&["multi", "wrapped"]);
}

#[test]
fn callbacks_keep_shebang_jsx_and_keyword_admission() {
    super::fixtures::witnesses(&["same"]);
    super::fixtures::inputs(&[
        "shebang",
        "shebang-late",
        "jsx-open",
        "jsx-self-close",
        "jsx-comparison",
        "jsx-generics",
        "jsx-no-close",
        "jsx-nested-type",
    ]);
}

#[test]
fn eof_and_zero_width_keep_progress_and_text() {
    super::fixtures::witnesses(&["zero"]);
    super::fixtures::inputs(&[
        "zero-input-lf",
        "zero-input-crlf",
        "only-bom",
        "only-nel",
        "unterminated-comment",
        "unterminated-string",
    ]);
}
