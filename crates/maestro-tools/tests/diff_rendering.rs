//! Public numbered diff rendering witnesses.
#![cfg(not(target_arch = "wasm32"))]
use crate::diff_support;
use crate::display_support;

#[test]
fn diff_raw_lines_preserve_framing() {
    diff_support::group("diff_raw_lines_preserve_framing");
}

#[test]
fn diff_line_fields_follow_display_grammar() {
    diff_support::group("diff_line_fields_follow_display_grammar");
}

#[test]
fn diff_standalone_rows_keep_order_and_tabs() {
    diff_support::group("diff_standalone_rows_keep_order_and_tabs");
}

#[test]
fn diff_only_single_line_pairs_use_inverse() {
    diff_support::group("diff_only_single_line_pairs_use_inverse");
}

#[test]
fn diff_changed_tokens_keep_context_and_indentation() {
    diff_support::group("diff_changed_tokens_keep_context_and_indentation");
}

#[test]
fn diff_replacement_whitespace_keeps_common_boundaries() {
    diff_support::group("diff_replacement_whitespace_keeps_common_boundaries");
}

#[test]
fn diff_insertions_deduplicate_surrounding_whitespace() {
    diff_support::group("diff_insertions_deduplicate_surrounding_whitespace");
}

#[test]
fn diff_deletions_reconcile_both_context_sides() {
    diff_support::group("diff_deletions_reconcile_both_context_sides");
}

#[test]
fn diff_edge_deletions_use_whitespace_overlap() {
    diff_support::group("diff_edge_deletions_use_whitespace_overlap");
}

#[test]
fn diff_word_boundaries_preserve_latin_ranges_and_punctuation() {
    diff_support::group("diff_word_boundaries_preserve_latin_ranges_and_punctuation");
}

#[test]
fn diff_whitespace_uses_the_shared_scalar_set() {
    diff_support::group("diff_whitespace_uses_the_shared_scalar_set");
}

#[test]
fn diff_ambiguous_word_runs_match_recorded_selection() {
    diff_support::group("diff_ambiguous_word_runs_match_recorded_selection");
}

#[test]
fn diff_options_do_not_select_syntax_highlighting() {
    diff_support::group("diff_options_do_not_select_syntax_highlighting");
}

#[test]
fn diff_inverse_preserves_embedded_style_boundaries() {
    diff_support::group("diff_inverse_preserves_embedded_style_boundaries");
}

#[test]
fn diff_theme_roles_follow_supplied_colors() {
    diff_support::group("diff_theme_roles_follow_supplied_colors");
}

#[test]
fn diff_default_themes_keep_visible_signs_and_role_colors() {
    diff_support::group("diff_default_themes_keep_visible_signs_and_role_colors");
}

#[test]
fn diff_live_theme_publication_and_failures_follow_the_shared_theme() {
    use display_support::{Env, NoLanguages, palette_of};
    use maestro_tools::{RenderDiffOptions, render_diff};
    use std::rc::Rc;
    let options = RenderDiffOptions::default();
    let diff = "raw\n-1 old\n+1 new\n 2 kept";
    assert_eq!(
        render_diff(diff, &options, &maestro_theme::LiveTheme::default())
            .unwrap_err()
            .to_string(),
        "Theme not initialized. Call initTheme() first."
    );
    let keys = ["toolDiffContext", "toolDiffRemoved", "toolDiffAdded"].map(str::to_owned);
    let env = Env::with(Rc::new(palette_of(&keys, 0).unwrap()), Rc::new(NoLanguages)).unwrap();
    assert_eq!(
        render_diff(diff, &options, &env.live).unwrap(),
        "\x1b[38;5;1mraw\x1b[39m\n\x1b[38;5;2m-1 \x1b[7mold\x1b[27m\x1b[39m\n\x1b[38;5;3m+1 \x1b[7mnew\x1b[27m\x1b[39m\n\x1b[38;5;1m 2 kept\x1b[39m"
    );
    env.state
        .set_theme_instance(Rc::new(palette_of(&keys, 10).unwrap()))
        .unwrap();
    assert_eq!(
        render_diff(diff, &options, &env.live).unwrap(),
        "\x1b[38;5;11mraw\x1b[39m\n\x1b[38;5;12m-1 \x1b[7mold\x1b[27m\x1b[39m\n\x1b[38;5;13m+1 \x1b[7mnew\x1b[27m\x1b[39m\n\x1b[38;5;11m 2 kept\x1b[39m"
    );
    env.state
        .set_theme_instance(Rc::new(palette_of(&keys[..2], 0).unwrap()))
        .unwrap();
    assert_eq!(
        render_diff(diff, &options, &env.live)
            .unwrap_err()
            .to_string(),
        "Unknown theme color: toolDiffAdded"
    );
}
