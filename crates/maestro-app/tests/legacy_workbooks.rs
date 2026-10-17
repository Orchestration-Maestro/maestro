//! Legacy workbook formatting through the public owned reader.
#![cfg(test)]
mod legacy_workbook_support;
mod workbook_fixture;

#[test]
fn legacy_producer_date_format_repairs_display_and_tables() {
    use maestro_app::browser::ssf::{self, CellValue, Ssf};
    let file = "legacy-producer-date.xls";
    assert!(
        native(file)
            .number_formats()
            .contains(&(60, "d.m".to_owned()))
    );
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

fn native(file: &str) -> calamine::Xls<std::io::Cursor<Vec<u8>>> {
    use calamine::Reader;
    calamine::Xls::new(std::io::Cursor::new(legacy_workbook_support::bytes(file))).unwrap()
}

fn formats(file: &str) -> Vec<((u32, u32), Option<u16>)> {
    native(file)
        .worksheet_cell_formats("Data")
        .unwrap()
        .iter()
        .map(|cell| (cell.position, cell.num_fmt_id))
        .collect()
}

#[test]
fn legacy_metadata_accessors_keep_absence_and_missing_sheet_errors() {
    let book = native("legacy-table-overwrite.xls");
    assert_eq!(
        book.number_formats(),
        &[
            (56, "\"上午/下午 \"hh\"時\"mm\"分\"ss\"秒 \"".to_owned()),
            (164, "\"first\"0".to_owned()),
            (500, "\"high\"0".to_owned()),
            (164, "\"last\"0".to_owned()),
            (168, "\"unused\"0".to_owned()),
        ]
    );
    assert!(
        matches!(book.worksheet_dimensions("missing"), Err(calamine::XlsError::WorksheetNotFound(name)) if name == "missing")
    );
    assert!(
        matches!(book.worksheet_cell_formats("missing"), Err(calamine::XlsError::WorksheetNotFound(name)) if name == "missing")
    );
    check_emitted_formats();
    for (file, expected) in [
        ("legacy-range-absent.xls", None),
        ("legacy-empty-zero.xls", Some(((0, 0), (0, 0)))),
        ("legacy-empty-one.xls", Some(((0, 0), (1, 1)))),
        ("legacy-declared-offset.xls", Some(((4, 2), (20, 8)))),
        ("legacy-empty-limit.xls", Some(((0, 0), (65536, 256)))),
    ] {
        assert_eq!(
            native(file)
                .worksheet_dimensions("Data")
                .unwrap()
                .map(|d| (d.start, d.end)),
            expected
        );
    }
}

fn check_emitted_formats() {
    assert_eq!(
        formats("legacy-record-duplicates.xls"),
        vec![
            ((8, 2), Some(164)),
            ((0, 0), Some(164)),
            ((8, 2), Some(165)),
        ]
    );
    assert_eq!(
        formats("legacy-record-kinds.xls"),
        vec![
            ((0, 0), Some(164)),
            ((0, 1), Some(164)),
            ((0, 2), Some(10)),
            ((0, 3), Some(164)),
            ((1, 0), Some(165)),
            ((1, 1), Some(165)),
            ((1, 2), Some(165)),
            ((2, 0), Some(10)),
            ((2, 1), Some(164)),
            ((3, 0), Some(164)),
            ((3, 1), Some(10)),
            ((3, 2), Some(164)),
            ((4, 0), Some(165)),
            ((4, 1), Some(165)),
        ]
    );
    assert_eq!(
        formats("legacy-unresolved-xf.xls"),
        vec![
            ((0, 0), Some(0)),
            ((0, 1), None),
            ((0, 2), None),
            ((0, 3), None),
            ((1, 0), None),
            ((1, 1), None),
            ((1, 2), Some(10)),
            ((2, 0), None),
        ]
    );
}
