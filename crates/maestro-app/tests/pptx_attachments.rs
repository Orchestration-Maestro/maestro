//! Presentation extraction through the public application boundary.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::support;
use maestro_app::browser::utils::attachment_utils::{AttachmentSource, load_attachment};
use maestro_models::Fetch;
use std::io::{Cursor, Write};
use std::sync::Arc;

/// One deflated slide whose headers declare `declared` expanded bytes.
fn slide_package(declared: u32) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    writer.start_file("ppt/slides/slide1.xml", options).unwrap();
    writer.write_all(b"<a:t>hi</a:t>").unwrap();
    let mut bytes = writer.finish().unwrap().into_inner();
    for (signature, offset) in [(b"PK\x03\x04", 22), (b"PK\x01\x02", 24)] {
        let at = bytes.windows(4).position(|w| w == signature).unwrap() + offset;
        bytes[at..at + 4].copy_from_slice(&declared.to_le_bytes());
    }
    bytes
}

#[test]
fn pptx_entries_past_the_size_bound_fail_without_inflating() {
    const BOUND: u32 = 512 << 20;
    for (declared, expected) in [
        (BOUND, Ok("hi")),
        (1, Err("File is larger than its declared uncompressed size")),
        (BOUND + 1, Err("entry is larger than 512 MiB")),
    ] {
        let fetch: Fetch = Arc::new(|_| panic!("local source fetched"));
        let result = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(load_attachment(
                AttachmentSource::Buffer(&slide_package(declared)),
                Some("a.pptx"),
                &fetch,
            ));
        match expected {
            Ok(text) => assert!(
                result.unwrap().extracted_text.unwrap().contains(text),
                "{declared}"
            ),
            Err(message) => assert_eq!(
                result.unwrap_err().to_string(),
                format!("Failed to process PPTX: {message}"),
                "{declared}"
            ),
        }
    }
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
