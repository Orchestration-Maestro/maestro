//! List numbering: counters, markers, resets, formats, level text and picture bullets.
use super::check_preview;

#[test]
fn numbering_levels_emit_markers_counters_and_resets() {
    check_preview("numbering-levels");
}

#[test]
fn unwrapped_numbering_resets_counters_on_the_root() {
    check_preview("numbering-levels-unwrapped");
}

#[test]
fn numbering_formats_map_to_list_style_types() {
    check_preview("numbering-formats");
}

#[test]
fn level_text_keeps_literals_and_genuine_placeholders() {
    check_preview("numbering-level-text");
}

#[test]
fn picture_bullets_define_their_image_after_the_pages() {
    check_preview("numbering-picture-bullet");
}

#[test]
fn numbering_paragraph_styles_take_the_level_that_names_them() {
    check_preview("numbering-style-level");
}

#[test]
fn numbering_rules_with_unusable_identifiers_are_skipped() {
    check_preview("numbering-id-escape");
}

#[test]
fn null_and_unreadable_numbering_values_keep_their_spelling() {
    check_preview("numbering-null-values");
}
