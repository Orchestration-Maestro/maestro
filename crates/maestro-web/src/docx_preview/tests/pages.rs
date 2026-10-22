//! Page composition: section groups, page breaks, geometry and columns.
use super::check;

#[test]
fn pages_use_section_margins_and_page_size() {
    check("single-section");
}

#[test]
fn section_properties_close_the_page_before_the_paragraph() {
    check("two-sections-page-break");
}

#[test]
fn last_rendered_page_breaks_are_ignored_by_default() {
    check("last-rendered-break");
}

#[test]
fn keeping_last_rendered_breaks_splits_pages() {
    check("last-rendered-break-kept");
}

#[test]
fn page_breaks_split_the_paragraph_at_the_break() {
    check("page-break-mid-paragraph");
}

#[test]
fn a_break_inside_a_run_keeps_the_trailing_content() {
    check("page-break-mid-run");
}

#[test]
fn a_trailing_break_starts_a_page_without_splitting() {
    check("page-break-last-child");
}

#[test]
fn break_paragraph_section_properties_keep_both_halves() {
    check("page-break-with-section-properties");
}

#[test]
fn orientation_and_size_changes_start_a_new_page() {
    check("orientation-change");
}

#[test]
fn a_size_change_alone_starts_a_new_page() {
    check("size-change-only");
}

#[test]
fn ignored_page_size_leaves_dimensions_unset() {
    check("width-change-ignored");
}

#[test]
fn a_page_break_before_style_starts_a_new_page() {
    check("style-page-break-before");
    check("style-page-break-before-after-section");
}

#[test]
fn columns_style_the_section_content() {
    check("columns");
}

#[test]
fn a_single_column_keeps_the_count_only() {
    check("columns-single");
}

#[test]
fn disabled_break_detection_keeps_one_page() {
    check("break-pages-disabled");
}

#[test]
fn an_empty_body_still_renders_one_page() {
    check("empty-body");
}

#[test]
fn body_section_properties_apply_to_every_page() {
    check("body-section-properties");
}

#[test]
fn section_and_page_breaks_split_at_both() {
    check("section-and-page-break-together");
}

#[test]
fn body_background_styles_the_page() {
    check("body-background");
}
