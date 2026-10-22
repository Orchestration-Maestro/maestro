//! Word extraction through the public application boundary.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::support::document_attachments::{WORD, suite};

#[test]
fn word_recognition_selects_mime_or_suffix() {
    suite(&WORD, "word_recognition_selects_mime_or_suffix");
}
#[test]
fn word_recognition_precedes_presentation_and_spreadsheet() {
    suite(
        &WORD,
        "word_recognition_precedes_presentation_and_spreadsheet",
    );
}
#[test]
fn word_paragraphs_extract_in_document_order() {
    suite(&WORD, "word_paragraphs_extract_in_document_order");
}
#[test]
fn word_source_routes_retain_original_metadata() {
    suite(&WORD, "word_source_routes_retain_original_metadata");
}
#[test]
fn word_page_wrapper_keeps_literal_filename() {
    suite(&WORD, "word_page_wrapper_keeps_literal_filename");
}
#[test]
fn word_body_walks_paragraphs_and_tables_in_order() {
    suite(&WORD, "word_body_walks_paragraphs_and_tables_in_order");
}
#[test]
fn word_runs_concatenate_without_added_separators() {
    suite(&WORD, "word_runs_concatenate_without_added_separators");
}
#[test]
fn word_run_padding_trims_at_element_edges() {
    suite(&WORD, "word_run_padding_trims_at_element_edges");
}
#[test]
fn word_empty_and_whitespace_paragraphs_are_skipped() {
    suite(&WORD, "word_empty_and_whitespace_paragraphs_are_skipped");
}
#[test]
fn word_tables_join_cells_and_rows() {
    suite(&WORD, "word_tables_join_cells_and_rows");
}
#[test]
fn word_table_cells_join_their_paragraphs() {
    suite(&WORD, "word_table_cells_join_their_paragraphs");
}
#[test]
fn word_table_rows_skip_empty_cells() {
    suite(&WORD, "word_table_rows_skip_empty_cells");
}
#[test]
fn word_table_cells_trim_each_element() {
    suite(&WORD, "word_table_cells_trim_each_element");
}
#[test]
fn word_nested_tables_extract_inline() {
    suite(&WORD, "word_nested_tables_extract_inline");
}
#[test]
fn word_hyperlinks_contribute_their_text() {
    suite(&WORD, "word_hyperlinks_contribute_their_text");
}
#[test]
fn word_smart_tags_contribute_their_text() {
    suite(&WORD, "word_smart_tags_contribute_their_text");
}
#[test]
fn word_tracked_insertions_extract_and_deletions_do_not() {
    suite(
        &WORD,
        "word_tracked_insertions_extract_and_deletions_do_not",
    );
}
#[test]
fn word_tab_and_break_markup_adds_no_text() {
    suite(&WORD, "word_tab_and_break_markup_adds_no_text");
}
#[test]
fn word_simple_field_results_extract() {
    suite(&WORD, "word_simple_field_results_extract");
}
#[test]
fn word_field_markers_and_bookmarks_add_no_text() {
    suite(&WORD, "word_field_markers_and_bookmarks_add_no_text");
}
#[test]
fn word_math_text_extracts() {
    suite(&WORD, "word_math_text_extracts");
}
#[test]
fn word_text_box_content_adds_no_text() {
    suite(&WORD, "word_text_box_content_adds_no_text");
}
#[test]
fn word_content_controls_extract_through_their_content() {
    suite(&WORD, "word_content_controls_extract_through_their_content");
}
#[test]
fn word_section_properties_add_no_text() {
    suite(&WORD, "word_section_properties_add_no_text");
}
#[test]
fn word_text_keeps_unicode_and_decoded_entities() {
    suite(&WORD, "word_text_keeps_unicode_and_decoded_entities");
}
#[test]
fn word_missing_main_part_yields_empty_page() {
    suite(&WORD, "word_missing_main_part_yields_empty_page");
}
#[test]
fn word_body_less_main_part_yields_empty_page() {
    suite(&WORD, "word_body_less_main_part_yields_empty_page");
}
#[test]
fn word_processing_failures_keep_kind_and_source() {
    suite(&WORD, "word_processing_failures_keep_kind_and_source");
}
