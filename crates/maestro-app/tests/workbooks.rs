//! File-backed public workbook projections.
#![cfg(test)]
use maestro_app::browser::ssf::{self, CellValue, ExcelDataType, Ssf, WorkbookError};
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
struct Position {
    r: u32,
    c: u32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Bounds {
    s: Position,
    e: Position,
}
impl Bounds {
    fn range(&self) -> ssf::Range {
        ssf::Range {
            s: ssf::CellAddress {
                r: self.s.r,
                c: self.s.c,
            },
            e: ssf::CellAddress {
                r: self.e.r,
                c: self.e.c,
            },
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Cell {
    address: String,
    r#type: String,
    value: Value,
    w_after_read: Option<String>,
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
            match error.kind.as_str() {
                "invalid_archive" => assert!(matches!(error_value, WorkbookError::Archive(_))),
                "invalid_workbook_xml" => assert!(matches!(error_value, WorkbookError::Xml(_))),
                "missing_package_part" => {
                    assert!(matches!(error_value, WorkbookError::MissingPart(_)));
                }
                _ => panic!("unexpected error kind"),
            }
            if let Some(part) = error.part {
                assert!(error_value.to_string().contains(&part));
            }
            assert!(case.record.is_none());
        }
        Outcome::Ok { ok } => check_book(result.unwrap(), case.record.unwrap(), ok, file, &ssf),
    }
}
fn check_book(mut book: ssf::WorkBook, record: Record, ok: Expected, file: &str, ssf: &Ssf) {
    assert_eq!(record.file, file);
    assert_eq!(book.date1904, record.date1904);
    assert_eq!(book.ssf, record.ssf.into_iter().collect::<BTreeMap<_, _>>());
    assert_eq!(
        book.sheets.keys().collect::<Vec<_>>(),
        ok.sheets.iter().map(|s| &s.name).collect::<Vec<_>>()
    );
    for expected in ok.sheets {
        let sheet = book.sheets.get_mut(&expected.name).unwrap();
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
    };
    assert_eq!(kind, expected.r#type);
    assert_eq!(cell.v, expected.value.scalar());
    assert_eq!(cell.w, expected.w_after_read);
    assert_eq!(ssf::format_cell(Some(cell), ssf), expected.text.scalar());
    assert_eq!(cell.w, expected.w_after_format);
    assert_eq!(cell.v, expected.value.scalar());
}

#[test]
fn workbook_sheet_order_and_scalar_text() {
    for file in ["basic.xlsx", "blank-stubs.xlsx", "styles-missing.xlsx"] {
        check(file);
    }
}

#[test]
fn workbook_numeric_codes_and_custom_sections() {
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
fn workbook_formulas_keep_only_reference_cached_values() {
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
