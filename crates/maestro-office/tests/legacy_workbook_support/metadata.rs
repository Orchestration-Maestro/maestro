//! Retained binary metadata and complete writer observations.
use super::Scalar;
use maestro_office::spreadsheet::{self, Ssf};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    file: String,
    test: String,
    expected: Outcome,
}
#[derive(Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum Outcome {
    Book { ok: Book },
    Error { error: Failure },
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Failure {
    kind: String,
    workbook: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Book {
    sheets: Vec<Sheet>,
    views: Vec<View>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Sheet {
    name: String,
    range: Option<String>,
    cells: Vec<Cell>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Cell {
    address: String,
    t: String,
    v: Scalar,
    w: Option<String>,
    f: Option<String>,
    #[serde(rename = "F")]
    array: Option<String>,
    l: Option<Link>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Link {
    #[serde(rename = "Target")]
    target: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct View {
    name: String,
    csv: String,
    html: String,
}

pub fn run(test: &str) {
    let cases: Vec<Case> = serde_json::from_str(
        &std::fs::read_to_string(format!(
            "{}/tests/fixtures/spreadsheet_formatting/legacy_metadata.json",
            env!("CARGO_MANIFEST_DIR")
        ))
        .expect("recorded fixture"),
    )
    .unwrap();
    let mut count = 0;
    for case in cases.iter().filter(|case| case.test == test) {
        count += 1;
        let mut ssf = Ssf::default();
        let result = spreadsheet::read(&super::bytes(&case.file), &case.file, &mut ssf);
        match &case.expected {
            Outcome::Book { ok } => check_book(result.unwrap(), ok, &ssf),
            Outcome::Error { error } => {
                assert_eq!(error.kind, "Reader");
                assert!(case.file.ends_with(&error.workbook));
                let error = result.unwrap_err();
                assert!(matches!(error, spreadsheet::WorkbookError::Reader(_)));
                assert!(error.to_string().contains(&case.file));
            }
        }
    }
    assert!(count > 0, "missing corpus group");
}

fn check_book(mut book: spreadsheet::WorkBook, expected: &Book, ssf: &Ssf) {
    check_sheets(&book, &expected.sheets);
    for view in &expected.views {
        let sheet = book.sheets.get_mut(&view.name).unwrap().as_mut().unwrap();
        assert_eq!(spreadsheet::sheet_to_csv(sheet, ssf), view.csv);
        assert_eq!(
            spreadsheet::sheet_to_html(sheet, &format!("sheet-{}", view.name), ssf),
            view.html
        );
        check_sheets(&book, &expected.sheets);
    }
}
fn check_sheets(book: &spreadsheet::WorkBook, expected: &[Sheet]) {
    assert_eq!(book.sheets.len(), expected.len());
    for ((name, actual), expected) in book.sheets.iter().zip(expected) {
        let actual = actual.as_ref().unwrap();
        assert_eq!(name, &expected.name);
        assert_eq!(actual.range.map(spreadsheet::encode_range), expected.range);
        assert_eq!(actual.cells.len(), expected.cells.len());
        for ((address, cell), expected) in actual.cells.iter().zip(&expected.cells) {
            assert_eq!(address, &expected.address);
            check_cell(cell, expected);
        }
    }
}

fn check_cell(cell: &spreadsheet::CellObject, expected: &Cell) {
    let kind = match cell.t.unwrap() {
        spreadsheet::ExcelDataType::Number => "n",
        spreadsheet::ExcelDataType::Boolean => "b",
        spreadsheet::ExcelDataType::Text => "s",
        spreadsheet::ExcelDataType::Error => "e",
        _ => panic!("unexpected cell type"),
    };
    assert_eq!(kind, expected.t);
    expected.v.check(&cell.v);
    assert_eq!(cell.w, expected.w);
    assert_eq!(cell.f, expected.f);
    assert_eq!(cell.array_formula, expected.array);
    assert_eq!(
        cell.l.as_ref().map(|l| l.target.as_str()),
        expected.l.as_ref().map(|l| l.target.as_str())
    );
    assert!(cell.h.is_none());
    assert!(cell.z.is_none());
    assert!(cell.xf.is_none());
}
