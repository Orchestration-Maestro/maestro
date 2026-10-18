//! Legacy workbook formatting through the public owned reader.
#![cfg(test)]
use crate::legacy_workbook_support;

#[test]
fn legacy_producer_date_format_repairs_display_and_tables() {
    use maestro_app::browser::ssf::{self, CellValue, Ssf};
    let file = "legacy-producer-date.xls";
    let mut ssf = Ssf::default();
    let book = ssf::read(&legacy_workbook_support::bytes(file), file, &mut ssf).unwrap();
    let cell = &book.sheets["Data"].as_ref().unwrap().cells["A1"];
    assert_eq!(cell.v, CellValue::Number(44927.0));
    assert_eq!(cell.w.as_deref(), Some("1.1"));
    assert_eq!(book.ssf[&60], "d\\.m");
    assert_eq!(ssf.get_table().read().unwrap()[&60], "d\\.m");
}

#[test]
fn legacy_biff5_empty_strings_keep_empty_cells_and_nonempty_controls() {
    use maestro_app::browser::ssf::{self, CellValue, Ssf};
    let file = "legacy-biff5-empty-string.xls";
    let book = ssf::read(
        &legacy_workbook_support::bytes(file),
        file,
        &mut Ssf::default(),
    )
    .unwrap();
    let sheet = book.sheets["Data"].as_ref().unwrap();
    assert_eq!(sheet.cells["A1"].v, CellValue::Text(String::new()));
    assert_eq!(sheet.cells["A2"].v, CellValue::Text("control".to_owned()));
    assert_eq!(sheet.cells.len(), 2);
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

#[test]
fn legacy_formulas_keep_each_cached_result_kind() {
    legacy_workbook_support::metadata::run("legacy_formulas_keep_each_cached_result_kind");
}

#[test]
fn legacy_formula_operators_keep_expression_order() {
    legacy_workbook_support::metadata::run("legacy_formula_operators_keep_expression_order");
}

#[test]
fn legacy_formula_unary_tokens_keep_delimiters() {
    legacy_workbook_support::metadata::run("legacy_formula_unary_tokens_keep_delimiters");
}

#[test]
fn legacy_formula_function_tokens_keep_argument_order() {
    legacy_workbook_support::metadata::run("legacy_formula_function_tokens_keep_argument_order");
}

#[test]
fn legacy_formula_literals_keep_numeric_and_string_spelling() {
    legacy_workbook_support::metadata::run(
        "legacy_formula_literals_keep_numeric_and_string_spelling",
    );
}

#[test]
fn legacy_formula_empty_expression_keeps_its_cache() {
    legacy_workbook_support::metadata::run("legacy_formula_empty_expression_keeps_its_cache");
}

#[test]
fn legacy_formula_coordinates_keep_absolute_axes() {
    legacy_workbook_support::metadata::run("legacy_formula_coordinates_keep_absolute_axes");
}

#[test]
fn legacy_formula_error_literals_keep_each_code() {
    legacy_workbook_support::metadata::run("legacy_formula_error_literals_keep_each_code");
}

#[test]
fn legacy_formula_number_boundaries_use_shared_spelling() {
    legacy_workbook_support::metadata::run("legacy_formula_number_boundaries_use_shared_spelling");
}

#[test]
fn legacy_formula_function_ids_keep_published_spellings() {
    legacy_workbook_support::metadata::run("legacy_formula_function_ids_keep_published_spellings");
}

#[test]
fn legacy_formula_fixed_functions_keep_each_argument_count() {
    legacy_workbook_support::metadata::run(
        "legacy_formula_fixed_functions_keep_each_argument_count",
    );
}

#[test]
fn legacy_writer_without_cache_retains_empty_text_result() {
    legacy_workbook_support::metadata::run("legacy_writer_without_cache_retains_empty_text_result");
}

#[test]
fn legacy_rich_strings_keep_plain_text_and_safe_preview() {
    legacy_workbook_support::metadata::run("legacy_rich_strings_keep_plain_text_and_safe_preview");
}

#[test]
fn legacy_formula_areas_keep_axes_and_error_references() {
    legacy_workbook_support::metadata::run("legacy_formula_areas_keep_axes_and_error_references");
}

#[test]
fn legacy_formula_control_and_memory_tokens_do_not_change_text() {
    legacy_workbook_support::metadata::run(
        "legacy_formula_control_and_memory_tokens_do_not_change_text",
    );
}

#[test]
fn legacy_formula_spacing_keeps_spaces_and_carriage_returns() {
    legacy_workbook_support::metadata::run(
        "legacy_formula_spacing_keeps_spaces_and_carriage_returns",
    );
}

#[test]
fn legacy_formula_array_constants_keep_rows_types_and_quotes() {
    legacy_workbook_support::metadata::run(
        "legacy_formula_array_constants_keep_rows_types_and_quotes",
    );
}

#[test]
fn legacy_formula_names_keep_identity_and_prefix_rules() {
    legacy_workbook_support::metadata::run("legacy_formula_names_keep_identity_and_prefix_rules");
}

#[test]
fn legacy_formula_sheet_references_quote_and_select_sheet_ranges() {
    legacy_workbook_support::metadata::run(
        "legacy_formula_sheet_references_quote_and_select_sheet_ranges",
    );
}

#[test]
fn legacy_formula_external_names_keep_book_selection() {
    legacy_workbook_support::metadata::run("legacy_formula_external_names_keep_book_selection");
}

#[test]
fn legacy_shared_formulas_expand_per_sheet_without_changing_caches() {
    legacy_workbook_support::metadata::run(
        "legacy_shared_formulas_expand_per_sheet_without_changing_caches",
    );
}

#[test]
fn legacy_array_formulas_keep_anchor_range_followers_and_cached_values() {
    legacy_workbook_support::metadata::run(
        "legacy_array_formulas_keep_anchor_range_followers_and_cached_values",
    );
}

#[test]
fn legacy_shared_formula_strings_keep_record_phase() {
    legacy_workbook_support::metadata::run("legacy_shared_formula_strings_keep_record_phase");
}

#[test]
fn legacy_array_formula_strings_keep_record_phase() {
    legacy_workbook_support::metadata::run("legacy_array_formula_strings_keep_record_phase");
}

#[test]
fn legacy_writer_shared_strings_keep_first_populated_anchor() {
    legacy_workbook_support::metadata::run(
        "legacy_writer_shared_strings_keep_first_populated_anchor",
    );
}

#[test]
fn legacy_writer_string_arrays_keep_cached_text_and_members() {
    legacy_workbook_support::metadata::run(
        "legacy_writer_string_arrays_keep_cached_text_and_members",
    );
}

#[test]
fn legacy_templates_keep_all_cache_kinds_and_unaffected_neighbors() {
    legacy_workbook_support::metadata::run(
        "legacy_templates_keep_all_cache_kinds_and_unaffected_neighbors",
    );
}

#[test]
fn legacy_array_membership_resets_per_sheet_and_checks_each_axis() {
    legacy_workbook_support::metadata::run(
        "legacy_array_membership_resets_per_sheet_and_checks_each_axis",
    );
}

#[test]
fn legacy_templates_keep_registration_order_and_cell_replacements() {
    legacy_workbook_support::metadata::run(
        "legacy_templates_keep_registration_order_and_cell_replacements",
    );
}

#[test]
fn legacy_formula_classes_keep_reference_results() {
    legacy_workbook_support::metadata::run("legacy_formula_classes_keep_reference_results");
}

#[test]
fn legacy_formula_relative_tokens_wrap_within_binary_grid() {
    legacy_workbook_support::metadata::run(
        "legacy_formula_relative_tokens_wrap_within_binary_grid",
    );
}

#[test]
fn legacy_biff5_formulas_use_their_own_layout() {
    legacy_workbook_support::metadata::run("legacy_biff5_formulas_use_their_own_layout");
}

#[test]
fn legacy_formula_defined_names_keep_scope_and_fallback() {
    legacy_workbook_support::metadata::run("legacy_formula_defined_names_keep_scope_and_fallback");
}

#[test]
fn legacy_defined_names_keep_all_builtin_spellings() {
    legacy_workbook_support::metadata::run("legacy_defined_names_keep_all_builtin_spellings");
}

#[test]
fn legacy_formula_external_sheets_keep_areas_and_deleted_ranges() {
    legacy_workbook_support::metadata::run(
        "legacy_formula_external_sheets_keep_areas_and_deleted_ranges",
    );
}

#[test]
fn legacy_formula_sheet_quoting_keeps_non_ascii_boundaries() {
    legacy_workbook_support::metadata::run(
        "legacy_formula_sheet_quoting_keeps_non_ascii_boundaries",
    );
}

#[test]
fn legacy_formula_extra_blocks_keep_token_order() {
    legacy_workbook_support::metadata::run("legacy_formula_extra_blocks_keep_token_order");
}

#[test]
fn legacy_formula_missing_arguments_and_optimized_sum_keep_shape() {
    legacy_workbook_support::metadata::run(
        "legacy_formula_missing_arguments_and_optimized_sum_keep_shape",
    );
}

#[test]
fn legacy_formula_ignored_tails_keep_completed_expressions() {
    legacy_workbook_support::metadata::run(
        "legacy_formula_ignored_tails_keep_completed_expressions",
    );
}

#[test]
fn legacy_formula_range_operators_keep_sheet_qualifiers() {
    legacy_workbook_support::metadata::run("legacy_formula_range_operators_keep_sheet_qualifiers");
}

#[test]
fn legacy_formula_typed_operands_keep_all_class_encodings() {
    legacy_workbook_support::metadata::run(
        "legacy_formula_typed_operands_keep_all_class_encodings",
    );
}

#[test]
fn legacy_formula_spacing_and_literals_keep_context() {
    legacy_workbook_support::metadata::run("legacy_formula_spacing_and_literals_keep_context");
}

#[test]
fn legacy_formula_unsupported_grammar_keeps_reader_failure() {
    legacy_workbook_support::metadata::run(
        "legacy_formula_unsupported_grammar_keeps_reader_failure",
    );
}

#[test]
fn legacy_older_operands_keep_name_area_and_sheet_layouts() {
    legacy_workbook_support::metadata::run(
        "legacy_older_operands_keep_name_area_and_sheet_layouts",
    );
}

#[test]
fn legacy_hyperlink_targets_keep_formatted_cells() {
    legacy_workbook_support::metadata::run("legacy_hyperlink_targets_keep_formatted_cells");
}

#[test]
fn legacy_hyperlink_ranges_affect_only_existing_cells() {
    legacy_workbook_support::metadata::run("legacy_hyperlink_ranges_affect_only_existing_cells");
}

#[test]
fn legacy_hyperlink_binary_branches_keep_target_selection() {
    legacy_workbook_support::metadata::run(
        "legacy_hyperlink_binary_branches_keep_target_selection",
    );
}

#[test]
fn legacy_hyperlink_record_order_keeps_replacement_semantics() {
    legacy_workbook_support::metadata::run(
        "legacy_hyperlink_record_order_keeps_replacement_semantics",
    );
}

#[test]
fn legacy_hyperlink_failures_keep_reader_context() {
    legacy_workbook_support::metadata::run("legacy_hyperlink_failures_keep_reader_context");
}

#[test]
fn legacy_hyperlink_optional_fields_keep_exact_target_identity() {
    legacy_workbook_support::metadata::run(
        "legacy_hyperlink_optional_fields_keep_exact_target_identity",
    );
}

#[test]
fn legacy_hyperlink_invalid_file_monikers_keep_reader_errors() {
    legacy_workbook_support::metadata::run(
        "legacy_hyperlink_invalid_file_monikers_keep_reader_errors",
    );
}

#[test]
fn legacy_biff5_negative_relative_rows_keep_version_grid() {
    use maestro_app::browser::ssf::{self, CellValue, Ssf};
    let file = "metadata-fix-biff5-negative.xls";
    let book = ssf::read(
        &legacy_workbook_support::bytes(file),
        file,
        &mut Ssf::default(),
    )
    .unwrap();
    let cell = &book.sheets.values().next().unwrap().as_ref().unwrap().cells["A1"];
    assert_eq!(cell.f.as_deref(), Some("$A16384"));
    assert_eq!(cell.v, CellValue::Number(7.0));
}

#[test]
fn legacy_shifted_areas_keep_whole_rows_in_each_operand_class() {
    use maestro_app::browser::ssf::{self, CellValue, Ssf};
    for class in [45, 77, 109] {
        let file = format!("metadata-fix-shifted-area-{class}.xls");
        let book = ssf::read(
            &legacy_workbook_support::bytes(&file),
            &file,
            &mut Ssf::default(),
        )
        .unwrap();
        let cell = &book.sheets["Data"].as_ref().unwrap().cells["C3"];
        assert_eq!(cell.f.as_deref(), Some("3:4"));
        assert_eq!(cell.v, CellValue::Number(7.0));
    }
}

#[test]
fn legacy_shared_areas_keep_whole_rows_in_each_operand_class() {
    use maestro_app::browser::ssf::{self, CellValue, Ssf};
    for class in [45, 77, 109] {
        let file = format!("metadata-fix-shared-area-{class}.xls");
        let book = ssf::read(
            &legacy_workbook_support::bytes(&file),
            &file,
            &mut Ssf::default(),
        )
        .unwrap();
        for address in ["A1", "A2"] {
            let cell = &book.sheets["Data"].as_ref().unwrap().cells[address];
            assert_eq!(cell.f.as_deref(), Some("SUM($1:$2)"));
            assert_eq!(cell.v, CellValue::Number(7.0));
        }
    }
}

#[test]
fn legacy_unused_external_sheet_layouts_keep_ordinary_cells_readable() {
    use maestro_app::browser::ssf::{self, CellValue, Ssf};
    for layout in ["old", "marker", "new", "control"] {
        let file = format!("metadata-fix-extern-{layout}.xls");
        let book = ssf::read(
            &legacy_workbook_support::bytes(&file),
            &file,
            &mut Ssf::default(),
        )
        .unwrap();
        assert_eq!(book.sheets["Data"].as_ref().unwrap().cells.len(), 1);
        let cell = &book.sheets["Data"].as_ref().unwrap().cells["A1"];
        assert_eq!(cell.v, CellValue::Number(23.0));
        assert!(cell.f.is_none());
    }
}

#[test]
fn legacy_range_prefixes_keep_complete_expression_identity() {
    use maestro_app::browser::ssf::{self, Ssf};
    let rows: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/ssf/legacy_range_prefixes.json")).unwrap();
    for row in rows.as_array().unwrap() {
        let file = row["file"].as_str().unwrap();
        let book = ssf::read(
            &legacy_workbook_support::bytes(file),
            file,
            &mut Ssf::default(),
        )
        .unwrap();
        assert_eq!(
            book.sheets[row["sheet"].as_str().unwrap()]
                .as_ref()
                .unwrap()
                .cells["A1"]
                .f
                .as_deref(),
            Some(row["formula"].as_str().unwrap()),
            "{file}"
        );
    }
}

#[test]
fn legacy_corrupt_sheet_stream_still_fails_workbook() {
    use maestro_app::browser::ssf::{self, CellValue, Ssf, WorkbookError};
    let book = ssf::read(
        &crate::workbook_fixture::bytes("binary-mixed-valid.xls"),
        "valid.xls",
        &mut Ssf::default(),
    )
    .unwrap();
    assert_eq!(
        book.sheets.keys().map(String::as_str).collect::<Vec<_>>(),
        ["Data", "Good"]
    );
    assert_eq!(
        book.sheets["Good"].as_ref().unwrap().cells["A1"].v,
        CellValue::Number(42.)
    );
    let error = ssf::read(
        &crate::workbook_fixture::bytes("binary-mixed-corrupt.xls"),
        "corrupt.xls",
        &mut Ssf::default(),
    )
    .unwrap_err();
    assert!(matches!(error, WorkbookError::Reader(_)));
}
