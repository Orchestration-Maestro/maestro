//! Legacy workbook formatting through the public owned reader.
#![cfg(test)]
use crate::legacy_workbook_support;

#[test]
fn legacy_producer_date_format_repairs_display_and_tables() {
    use maestro_app::browser::ssf::{self, CellValue, Ssf};
    let file = "legacy-producer-date.xls";
    let mut ssf = Ssf::default();
    let book = ssf::read(&legacy_workbook_support::bytes(file), file, &mut ssf).unwrap();
    let cell = &book.sheets["Data"].cells["A1"];
    assert_eq!(cell.v, CellValue::Number(44927.0));
    assert_eq!(cell.w.as_deref(), Some("1.1"));
    assert_eq!(book.ssf[&60], "d\\.m");
    assert_eq!(ssf.get_table().read().unwrap()[&60], "d\\.m");
}

#[test]
fn legacy_builtins_use_shared_format_engine() {
    legacy_workbook_support::run(&["legacy-builtins.xls"]);
}

#[test]
fn legacy_custom_sections_preserve_text_and_unicode() {
    legacy_workbook_support::run(&["legacy-sections.xls"]);
}

#[test]
fn legacy_epoch_formats_keep_serial_values() {
    legacy_workbook_support::run(&["legacy-epoch-false.xls", "legacy-epoch-true.xls"]);
}

#[test]
fn legacy_cell_record_kinds_select_their_own_format() {
    legacy_workbook_support::run(&["legacy-record-kinds.xls"]);
}

#[test]
fn legacy_failed_formats_leave_cache_empty_until_public_fallback() {
    legacy_workbook_support::run(&["legacy-format-failure.xls"]);
}

#[test]
fn legacy_empty_and_conditional_formats_preserve_zero_and_empty_text() {
    legacy_workbook_support::run(&["legacy-empty-and-conditions.xls"]);
}

#[test]
fn legacy_missing_xfs_keep_default_and_uncached_branches() {
    legacy_workbook_support::run(&["legacy-unresolved-xf.xls"]);
}

#[test]
fn legacy_format_records_load_in_order_and_keep_high_identifiers() {
    legacy_workbook_support::run(&["legacy-table-overwrite.xls"]);
}

#[test]
fn legacy_format_records_decode_compressed_unicode_and_codepage_text() {
    legacy_workbook_support::run(&["legacy-format-encodings.xls", "legacy-biff5.xls"]);
}

#[test]
fn legacy_records_keep_input_order() {
    legacy_workbook_support::run(&["legacy-record-order.xls", "legacy-record-duplicates.xls"]);
}

#[test]
fn legacy_live_tables_keep_aliases_and_detached_snapshots() {
    legacy_workbook_support::run(&["state-27", "state-28"]);
}

#[test]
fn legacy_declared_rectangles_include_blank_rows_and_columns() {
    legacy_workbook_support::run(&[
        "legacy-declared-wide.xls",
        "legacy-declared-offset.xls",
        "legacy-empty-wide.xls",
        "legacy-empty-limit.xls",
        "legacy-empty-one.xls",
        "legacy-empty-zero.xls",
    ]);
}

#[test]
fn legacy_ranges_union_cells_and_handle_missing_dimensions() {
    legacy_workbook_support::run(&[
        "legacy-range-union.xls",
        "legacy-range-absent.xls",
        "legacy-range-absent-empty.xls",
    ]);
}

#[test]
fn compound_stream_sizes_keep_identical_workbooks() {
    legacy_workbook_support::run_binary(&[
        "compound-stream-1821.xls",
        "compound-stream-4095.xls",
        "compound-stream-4096.xls",
        "compound-stream-4097.xls",
        "compound-stream-8192.xls",
    ]);
}

#[test]
fn compound_workbook_aliases_preserve_sheet_results() {
    legacy_workbook_support::run_binary(&[
        "compound-name-Book.xls",
        "compound-name-WORKBOOK.xls",
        "compound-name-BOOK.xls",
        "compound-name-wOrKbOoK.xls",
    ]);
}

#[test]
fn compound_allocation_extension_sectors_extend_the_table() {
    legacy_workbook_support::run_binary(&["compound-allocation-extension.xls"]);
}

#[test]
fn compound_fragmented_chains_preserve_stream_order() {
    legacy_workbook_support::run_binary(&[
        "compound-fragmented-fat.xls",
        "compound-fragmented-minifat.xls",
    ]);
}

#[test]
fn compound_sector_versions_keep_identical_values() {
    legacy_workbook_support::run_binary(&["compound-sector-4096.xls"]);
}

#[test]
fn compound_header_failures_keep_workbook_context() {
    legacy_workbook_support::run_binary(&[
        "compound-short-compound.xls",
        "compound-bad-major.xls",
        "compound-bad-sector-shift.xls",
        "compound-bad-mini-shift.xls",
    ]);
}

#[test]
fn compound_missing_stream_fails_without_partial_result() {
    legacy_workbook_support::run_binary(&["compound-no-workbook.xls"]);
}

#[test]
fn compound_workbook_selection_ignores_nested_storages() {
    legacy_workbook_support::run_binary(&["compound-nested-decoy.xls"]);
}

#[test]
fn compound_short_final_sector_keeps_stream() {
    legacy_workbook_support::run_binary(&["compound-short-final-sector.xls"]);
}

#[test]
fn legacy_shared_strings_continue_across_width_changes() {
    legacy_workbook_support::run_binary(&["legacy-sst-switch-width.xls"]);
}

#[test]
fn legacy_shared_strings_keep_surrogate_pairs_and_skip_rich_payloads() {
    legacy_workbook_support::run_binary(&[
        "legacy-sst-surrogate-boundary.xls",
        "legacy-sst-rich-extension.xls",
    ]);
}

#[test]
fn legacy_compressed_unicode_ignores_ansi_codepage() {
    legacy_workbook_support::run_binary(&["legacy-sst-compressed.xls"]);
}

#[test]
fn legacy_rk_storage_kinds_keep_numeric_bits() {
    legacy_workbook_support::run_binary(&["legacy-rk-floating.xls"]);
}

#[test]
fn legacy_encrypted_workbooks_fail_as_reader_errors() {
    legacy_workbook_support::run_binary(&["legacy-protected.xls"]);
}

#[test]
fn legacy_declared_code_pages_decode_text_without_mojibake() {
    legacy_workbook_support::run_binary(&[
        "legacy-biff5-windows-1252.xls",
        "legacy-biff5-shift-jis.xls",
    ]);
}

#[test]
fn legacy_code_page_aliases_and_unmapped_pages_decode_bytes() {
    legacy_workbook_support::run_binary(&["legacy-biff5-alias.xls", "legacy-biff5-unmapped.xls"]);
}

#[test]
fn legacy_embedded_chart_substreams_keep_sheet_cells() {
    legacy_workbook_support::run_binary(&["legacy-nested-chart.xls"]);
}

#[test]
fn legacy_chart_sheets_shift_repeated_numbers() {
    legacy_workbook_support::run_binary(&["legacy-chart-sheet.xls"]);
}

#[test]
fn legacy_sheet_order_follows_stream_positions() {
    legacy_workbook_support::run_binary(&["legacy-sheet-order.xls"]);
}

#[test]
fn legacy_formula_caches_keep_result_kinds() {
    legacy_workbook_support::run_binary(&["legacy-formula-caches.xls"]);
}

#[test]
fn legacy_boolean_and_error_records_keep_exact_flags() {
    legacy_workbook_support::run_binary(&["legacy-boolerr-flags.xls"]);
}

#[test]
fn legacy_sheets_without_dimensions_start_unbounded() {
    legacy_workbook_support::run_binary(&["legacy-range-per-sheet.xls"]);
}
