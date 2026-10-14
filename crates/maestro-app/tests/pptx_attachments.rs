//! Presentation extraction through the public application boundary.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
mod support {
    /// Shared complete attachment observations.
    pub mod attachment_observation;
    /// Presentation corpus replay.
    pub mod pptx;
}

#[test]
fn pptx_detection_selects_mime_or_suffix() {
    support::pptx::suite("pptx_detection_selects_mime_or_suffix");
}

#[test]
fn pptx_source_routes_retain_original_metadata() {
    support::pptx::suite("pptx_source_routes_retain_original_metadata");
}

#[test]
fn pptx_empty_package_keeps_outer_wrapper() {
    support::pptx::suite("pptx_empty_package_keeps_outer_wrapper");
}

#[test]
fn pptx_slides_use_numeric_order() {
    support::pptx::suite("pptx_slides_use_numeric_order");
}

#[test]
fn pptx_numeric_ties_keep_archive_order() {
    support::pptx::suite("pptx_numeric_ties_keep_archive_order");
}

#[test]
fn pptx_duplicate_names_replace_without_reordering() {
    support::pptx::suite("pptx_duplicate_names_replace_without_reordering");
}

#[test]
fn pptx_entry_selection_keeps_suffix_and_ascii_digits() {
    support::pptx::suite("pptx_entry_selection_keeps_suffix_and_ascii_digits");
}

#[test]
fn pptx_rooted_names_read_exact_entries() {
    support::pptx::suite("pptx_rooted_names_read_exact_entries");
}

#[test]
fn pptx_archive_names_reuse_dot_sanitization() {
    support::pptx::suite("pptx_archive_names_reuse_dot_sanitization");
}

#[test]
fn pptx_blank_slides_still_count_for_ordinals() {
    support::pptx::suite("pptx_blank_slides_still_count_for_ordinals");
}

#[test]
fn pptx_empty_runs_and_whitespace_have_distinct_wrappers() {
    support::pptx::suite("pptx_empty_runs_and_whitespace_have_distinct_wrappers");
}

#[test]
fn pptx_runs_keep_source_order_and_padding() {
    support::pptx::suite("pptx_runs_keep_source_order_and_padding");
}

#[test]
fn pptx_text_matching_keeps_literal_prefix_rules() {
    support::pptx::suite("pptx_text_matching_keeps_literal_prefix_rules");
}

#[test]
fn pptx_raw_matching_does_not_validate_whole_xml() {
    support::pptx::suite("pptx_raw_matching_does_not_validate_whole_xml");
}

#[test]
fn pptx_run_filter_uses_shared_whitespace_scalars() {
    support::pptx::suite("pptx_run_filter_uses_shared_whitespace_scalars");
}

#[test]
fn pptx_notes_follow_slides_in_numeric_order() {
    support::pptx::suite("pptx_notes_follow_slides_in_numeric_order");
}

#[test]
fn pptx_notes_keep_number_spelling_and_tie_order() {
    support::pptx::suite("pptx_notes_keep_number_spelling_and_tie_order");
}

#[test]
fn pptx_empty_notes_still_emit_notes_wrapper() {
    support::pptx::suite("pptx_empty_notes_still_emit_notes_wrapper");
}

#[test]
fn pptx_notes_join_runs_without_trimming() {
    support::pptx::suite("pptx_notes_join_runs_without_trimming");
}

#[test]
fn pptx_entities_decode_once_after_run_filter() {
    support::pptx::suite("pptx_entities_decode_once_after_run_filter");
}

#[test]
fn pptx_invalid_entity_fragments_keep_original_run() {
    support::pptx::suite("pptx_invalid_entity_fragments_keep_original_run");
}

#[test]
fn pptx_entity_decoding_keeps_literal_line_endings() {
    support::pptx::suite("pptx_entity_decoding_keeps_literal_line_endings");
}

#[test]
fn pptx_filenames_remain_literal_in_wrapper() {
    support::pptx::suite("pptx_filenames_remain_literal_in_wrapper");
}

#[test]
fn pptx_stored_and_deflated_parts_keep_same_text() {
    support::pptx::suite("pptx_stored_and_deflated_parts_keep_same_text");
}

#[test]
fn pptx_processing_errors_keep_kind_and_source() {
    support::pptx::suite("pptx_processing_errors_keep_kind_and_source");
}

#[test]
fn pptx_url_failures_precede_package_processing() {
    support::pptx::suite("pptx_url_failures_precede_package_processing");
}

#[test]
fn pptx_mime_matching_is_exact() {
    support::pptx::suite("pptx_mime_matching_is_exact");
}

#[test]
fn pptx_part_text_keeps_bom_inside_runs() {
    support::pptx::suite("pptx_part_text_keeps_bom_inside_runs");
}

#[test]
fn pptx_unselected_parts_are_not_decompressed() {
    support::pptx::suite("pptx_unselected_parts_are_not_decompressed");
}
