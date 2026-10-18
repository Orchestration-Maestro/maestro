//! File-backed public workbook projections.
#![cfg(test)]
use crate::workbook_fixture::Bounds;
use maestro_app::browser::ssf::{
    self, CellFormatOptions, CellValue, ExcelDataType, Ssf, WorkbookError,
};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    file: String,
    expected: Outcome,
    record: Option<Record>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    file: String,
    date1904: bool,
    ssf: Vec<(u32, String)>,
}
#[derive(Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum Outcome {
    Ok { ok: Expected },
    Error { error: Error },
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Error {
    kind: String,
    part: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    sheets: Vec<Sheet>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Sheet {
    name: String,
    range: Option<String>,
    merges: Vec<Bounds>,
    cells: Vec<Cell>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Cell {
    address: String,
    r#type: String,
    value: Value,
    w_after_read: Option<String>,
    f: Option<String>,
    array_formula: Option<String>,
    text: Value,
    w_after_format: Option<String>,
}
#[derive(Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum Value {
    Number { number: String },
    Empty { missing: bool },
    Text(String),
    Bool(bool),
}
impl Value {
    fn scalar(&self) -> CellValue {
        match self {
            Self::Number { number } => CellValue::Number(number.parse().unwrap()),
            Self::Empty { missing } => {
                assert!(*missing);
                CellValue::Missing
            }
            Self::Text(s) => CellValue::Text(s.clone()),
            Self::Bool(b) => CellValue::Boolean(*b),
        }
    }
}
fn check(file: &str) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/workbooks");
    let case: Case =
        serde_json::from_slice(&std::fs::read(path.join(format!("{file}.json"))).unwrap()).unwrap();
    assert_eq!(case.file, file);
    let mut ssf = Ssf::default();
    let result = ssf::read(&std::fs::read(path.join(file)).unwrap(), file, &mut ssf);
    match case.expected {
        Outcome::Error { error } => {
            let error_value = result.unwrap_err();
            assert!(error_value.to_string().contains(file));
            check_error(&error, &error_value);
            assert!(case.record.is_none());
        }
        Outcome::Ok { ok } => check_book(result.unwrap(), case.record.unwrap(), ok, file, &ssf),
    }
}
fn check_book(mut book: ssf::WorkBook, record: Record, ok: Expected, file: &str, ssf: &Ssf) {
    assert_eq!(record.file, file);
    assert_eq!(book.date1904, record.date1904);
    assert_eq!(book.ssf, record.ssf.into_iter().collect::<BTreeMap<_, _>>());
    assert_eq!(*ssf.get_table().read().unwrap(), book.ssf);
    assert_eq!(
        book.sheets.keys().collect::<Vec<_>>(),
        ok.sheets.iter().map(|s| &s.name).collect::<Vec<_>>()
    );
    for expected in ok.sheets {
        let sheet = book
            .sheets
            .get_mut(&expected.name)
            .unwrap()
            .as_mut()
            .unwrap();
        assert_eq!(sheet.range.map(ssf::encode_range), expected.range);
        assert_eq!(
            sheet.merges,
            expected
                .merges
                .iter()
                .map(Bounds::range)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            sheet.cells.keys().collect::<Vec<_>>(),
            expected
                .cells
                .iter()
                .map(|c| &c.address)
                .collect::<Vec<_>>()
        );
        for expected in expected.cells {
            check_cell(
                sheet.cells.get_mut(&expected.address).unwrap(),
                &expected,
                ssf,
            );
        }
    }
}

fn check_cell(cell: &mut ssf::CellObject, expected: &Cell, ssf: &Ssf) {
    let kind = match cell.t.unwrap() {
        ExcelDataType::Number => "n",
        ExcelDataType::Text => "s",
        ExcelDataType::Boolean => "b",
        ExcelDataType::Error => "e",
        ExcelDataType::Stub => "z",
        ExcelDataType::Date => "d",
    };
    assert_eq!(kind, expected.r#type);
    assert_scalar(&cell.v, &expected.value.scalar());
    assert_eq!(cell.w, expected.w_after_read);
    assert_eq!(cell.f, expected.f);
    assert_eq!(cell.array_formula, expected.array_formula);
    assert_scalar(
        &ssf::format_cell(Some(cell), None, &CellFormatOptions::default(), ssf),
        &expected.text.scalar(),
    );
    assert_eq!(cell.w, expected.w_after_format);
    assert_scalar(&cell.v, &expected.value.scalar());
}

#[test]
fn workbook_sheet_order_and_scalar_text() {
    for file in ["basic.xlsx", "blank-stubs.xlsx", "styles-missing.xlsx"] {
        check(file);
    }
}

#[test]
fn workbook_numeric_codes_and_custom_sections() {
    ssf_support::run("workbook_numeric_codes_and_custom_sections");
    for file in [
        "formats.xlsx",
        "custom.xlsx",
        "escaped-custom.xlsx",
        "high-custom-id.xlsx",
        "style-index-fallback.xlsx",
        "invalid-format.xlsx",
        "producer-format-fix.xlsx",
        "implicit-builtin.xlsx",
        "reader-dispatch.xlsx",
    ] {
        check(file);
    }
}

#[test]
fn workbook_date_epochs_and_iso_inputs() {
    for file in [
        "epoch-false.xlsx",
        "epoch-true.xlsx",
        "iso-dates.xlsx",
        "iso-1900-boundary.xlsx",
        "iso-1904.xlsx",
    ] {
        check(file);
    }
}

#[test]
fn workbook_ranges_and_merged_regions() {
    for file in [
        "merges.xlsx",
        "sparse.xlsx",
        "dimension-expanded.xlsx",
        "dimension-absent.xlsx",
    ] {
        check(file);
    }
}

#[test]
fn workbook_optional_and_missing_style_parts() {
    for file in ["no-styles-declaration.xlsx", "no-styles-part.xlsx"] {
        check(file);
    }
}

#[test]
fn workbook_raw_errors_survive_reader_conversion() {
    for file in ["errors.xlsx", "unknown-error.xlsx"] {
        check(file);
    }
}

#[test]
fn workbook_formulas_preserve_cached_and_uncached_values() {
    for file in ["formula.xlsx", "formula-no-cache.xlsx"] {
        check(file);
    }
}

#[test]
fn workbook_archive_and_xml_failures_keep_context() {
    for file in ["truncated.xlsx", "malformed-style.xlsx"] {
        check(file);
    }
}

#[test]
fn workbook_date_cells_accept_date_local_datetime_and_time_forms() {
    for file in ["iso-forms-False.xlsx", "iso-forms-True.xlsx"] {
        check(file);
    }
}

#[test]
fn workbook_time_only_cells_keep_epoch_serial_precision_and_general_text() {
    for file in ["clocks-false.xlsx", "clocks-true.xlsx"] {
        check(file);
    }
}

#[test]
fn workbook_blank_templates_keep_declared_rectangles() {
    for file in ["blank-template.xlsx", "blank-default.xlsx"] {
        check(file);
    }
}

#[test]
fn workbook_error_cells_load_implicit_formats_into_live_and_snapshot_tables() {
    check("error-implicit-format.xlsx");
}

#[test]
fn workbook_inline_multiline_strings_normalize_without_redecoding() {
    check("inline-multiline.xlsx");
}

#[test]
fn workbook_string_escapes_decode_once_without_redecoding_shared_text() {
    for file in ["escaped-raw.xlsx", "escaped-shared.xlsx"] {
        check(file);
    }
}

use crate::legacy_workbook_support;
use crate::ssf_support;

#[test]
fn workbook_legacy_values_follow_formats_and_declared_bounds() {
    legacy_workbook_support::run(&[
        "legacy-basic.xls",
        "legacy-custom.xls",
        "legacy-dimensions.xls",
    ]);
}

#[test]
fn workbook_legacy_projection_preserves_order_merges_epoch_and_raw_text() {
    legacy_workbook_support::run(&["legacy-states.xls"]);
}

#[test]
fn workbook_legacy_general_does_not_consume_active_format_zero() {
    ssf_support::run("workbook_legacy_general_does_not_consume_active_format_zero");
}

#[test]
fn workbook_legacy_reader_failure_keeps_workbook_context() {
    legacy_workbook_support::run(&["legacy-truncated.xls"]);
}

#[test]
fn workbook_format_code_escapes_share_one_decoder() {
    ssf_support::run("workbook_format_code_escapes_share_one_decoder");
}

fn assert_scalar(actual: &CellValue, expected: &CellValue) {
    if let (CellValue::Number(a), CellValue::Number(b)) = (actual, expected)
        && a.is_nan()
        && b.is_nan()
    {
        return;
    }
    assert_eq!(actual, expected);
}

#[test]
fn workbook_numeric_values_follow_decimal_prefix_rules() {
    check("numeric-prefixes.xlsx");
    let mut ssf = Ssf::default();
    let book = ssf::read(
        &legacy_workbook_support::bytes("numeric-prefixes.xlsx"),
        "numeric-prefixes.xlsx",
        &mut ssf,
    )
    .unwrap();
    let CellValue::Number(number) = book.sheets["Data"].as_ref().unwrap().cells["B1"].v else {
        panic!("expected number")
    };
    assert!(number.is_sign_negative());
}

#[test]
fn workbook_boolean_tokens_keep_exact_defaults() {
    check("boolean-tokens.xlsx");
}

#[test]
fn workbook_missing_coordinates_follow_row_and_cell_progression() {
    check("inferred-positions.xlsx");
}

#[test]
fn workbook_string_values_share_one_decoding_owner() {
    check("string-branches.xlsx");
}

#[test]
fn workbook_corrupt_shared_string_indices_fail_as_reader_errors() {
    check("bad-shared-index.xlsx");
}

fn check_error(error: &Error, error_value: &WorkbookError) {
    match error.kind.as_str() {
        "reader" => assert!(matches!(error_value, WorkbookError::Reader(_))),
        "invalid_archive" => assert!(matches!(error_value, WorkbookError::Archive(_))),
        "invalid_workbook_xml" => assert!(matches!(error_value, WorkbookError::Xml(_))),
        "missing_package_part" => {
            assert!(matches!(error_value, WorkbookError::MissingPart(_)));
        }
        _ => panic!("unexpected error kind"),
    }
    if let Some(part) = &error.part {
        assert!(error_value.to_string().contains(part));
    }
}

/// Read a qualified byte fixture through the public workbook boundary.
fn partial_book(file: &str) -> Result<ssf::WorkBook, WorkbookError> {
    let bytes = crate::workbook_fixture::bytes(file);
    let mut ssf = Ssf::default();
    let book = ssf::read(&bytes, file, &mut ssf)?;
    assert!(!book.date1904);
    assert_eq!(*ssf.get_table().read().unwrap(), book.ssf);
    Ok(book)
}
#[test]
fn workbook_mixed_sheet_results_preserve_order_and_values() {
    for file in [
        "mixed-bad-first.xlsx",
        "mixed-bad-last.xlsx",
        "mixed-valid-control.xlsx",
    ] {
        let result = partial_book(file);
        assert!(
            result.is_ok(),
            "readable sheets must survive a corrupt sibling: {file}"
        );
        let book = result.unwrap();
        let names = if file == "mixed-bad-last.xlsx" {
            ["Good", "Data"]
        } else {
            ["Data", "Good"]
        };
        assert_eq!(
            book.sheets.keys().map(String::as_str).collect::<Vec<_>>(),
            names
        );
        let good = book.sheets["Good"].as_ref().unwrap();
        assert_eq!(good.cells["A1"].v, CellValue::Number(42.));
        assert_eq!(good.cells["A1"].w.as_deref(), Some("42"));
        if file == "mixed-valid-control.xlsx" {
            assert_eq!(
                book.sheets["Data"].as_ref().unwrap().cells["A1"].v,
                CellValue::Text("one".into())
            );
        } else {
            assert!(matches!(book.sheets["Data"], Err(WorkbookError::Reader(_))));
        }
    }
}

#[test]
fn workbook_failed_sheet_discards_partial_cells() {
    let book = partial_book("mixed-partial-bad.xlsx").unwrap();
    assert!(matches!(book.sheets["Data"], Err(WorkbookError::Reader(_))));
    assert_eq!(
        book.sheets["Good"].as_ref().unwrap().cells["A1"].v,
        CellValue::Number(42.)
    );
}

#[test]
fn workbook_empty_valid_sibling_is_success() {
    let book = partial_book("mixed-empty-valid.xlsx").unwrap();
    assert!(matches!(book.sheets["Data"], Err(WorkbookError::Reader(_))));
    assert!(book.sheets["Good"].as_ref().unwrap().cells.is_empty());
}

#[test]
fn workbook_all_corrupt_sheets_keep_first_typed_error() {
    for file in ["bad-shared-index.xlsx", "all-corrupt-two.xlsx"] {
        let error = partial_book(file).unwrap_err();
        assert!(matches!(error, WorkbookError::Reader(_)));
        assert_eq!(
            error.to_string(),
            format!("{file}: xl/worksheets/sheet1.xml: shared string 3")
        );
    }
}

#[test]
fn workbook_missing_sheet_part_is_local() {
    let book = partial_book("missing-sheet-part.xlsx").unwrap();
    assert_eq!(
        book.sheets.keys().map(String::as_str).collect::<Vec<_>>(),
        ["Data", "Good"]
    );
    let error = book.sheets["Data"].as_ref().unwrap_err();
    assert!(matches!(error, WorkbookError::MissingPart(_)));
    assert!(error.to_string().contains("xl/worksheets/missing.xml"));
    assert_eq!(
        book.sheets["Good"].as_ref().unwrap().cells["A1"].v,
        CellValue::Number(42.)
    );
}

#[test]
fn workbook_many_sheet_results_keep_input_order() {
    for (file, count) in [
        ("sheets-32.xlsx", 32),
        ("sheets-64.xlsx", 64),
        ("relationships-many-32.xlsx", 32),
        ("relationships-many-64.xlsx", 64),
    ] {
        let book = partial_book(file).unwrap();
        assert_eq!(book.sheets.len(), count);
        for (i, (name, sheet)) in book.sheets.into_iter().enumerate() {
            assert_eq!(name, format!("Sheet{i}"));
            assert_eq!(
                sheet.unwrap().cells["A1"].v,
                CellValue::Number(f64::from(u32::try_from(i).unwrap()))
            );
        }
    }
    let bytes = include_bytes!("fixtures/spreadsheet/operand-grammar.xlsx");
    let book = ssf::read(bytes, "largest.xlsx", &mut Ssf::default()).unwrap();
    assert_eq!(book.sheets.len(), 433);
    assert!(book.sheets.values().all(Result::is_ok));
}

#[test]
fn workbook_relationship_duplicate_ids_keep_last_target() {
    for (file, expected) in [
        ("relationships-duplicate-last.xlsx", CellValue::Number(42.)),
        (
            "relationships-duplicate-first.xlsx",
            CellValue::Text("one".into()),
        ),
        (
            "relationships-first-empty-target.xlsx",
            CellValue::Text("one".into()),
        ),
    ] {
        let book = partial_book(file).unwrap();
        assert_eq!(
            book.sheets["Data"].as_ref().unwrap().cells["A1"].v,
            expected
        );
        assert_eq!(
            book.sheets["Good"].as_ref().unwrap().cells["A1"].v,
            CellValue::Number(42.)
        );
    }
}

#[test]
fn workbook_relationship_fallback_applies_to_every_sheet() {
    for file in [
        "relationships-missing-id.xlsx",
        "relationships-missing-sheet-id.xlsx",
        "relationships-empty-id.xlsx",
        "relationships-unknown-id.xlsx",
        "relationships-case-id.xlsx",
        "relationships-no-records.xlsx",
        "relationships-absent-file.xlsx",
        "relationships-fallback-all.xlsx",
        "relationships-zero-based-fallback.xlsx",
    ] {
        let book = partial_book(file).unwrap();
        assert_eq!(
            book.sheets.keys().map(String::as_str).collect::<Vec<_>>(),
            ["Data", "Good"]
        );
        assert_eq!(
            book.sheets["Data"].as_ref().unwrap().cells["A1"].v,
            CellValue::Text("one".into())
        );
        assert_eq!(
            book.sheets["Good"].as_ref().unwrap().cells["A1"].v,
            CellValue::Number(42.)
        );
    }
}

#[test]
fn workbook_missing_relationship_target_fails_globally() {
    for file in [
        "relationships-missing-target.xlsx",
        "relationships-unreferenced-missing-target.xlsx",
    ] {
        let error = partial_book(file).unwrap_err();
        assert!(matches!(error, WorkbookError::MissingPart(_)));
        assert_eq!(error.to_string(), format!("{file}: relationship target"));
    }
}

#[test]
fn workbook_empty_relationship_target_is_a_sheet_error() {
    for file in [
        "relationships-empty-target.xlsx",
        "relationships-last-empty-target.xlsx",
    ] {
        let book = partial_book(file).unwrap();
        assert_eq!(
            book.sheets.keys().map(String::as_str).collect::<Vec<_>>(),
            ["Data", "Good"]
        );
        let error = book.sheets["Data"].as_ref().unwrap_err();
        assert!(matches!(error, WorkbookError::MissingPart(_)));
        assert_eq!(error.to_string(), format!("{file}: sheet Data"));
        assert_eq!(
            book.sheets["Good"].as_ref().unwrap().cells["A1"].v,
            CellValue::Number(42.)
        );
    }
}
