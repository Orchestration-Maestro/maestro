//! Complete observations of legacy workbook reads and live format tables.
use maestro_app::browser::ssf::{self, CellFormatOptions, CellValue, Ssf};
use serde::Deserialize;
use std::{collections::BTreeMap, sync::Arc};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    op: String,
    args: Args,
    expected: Outcome,
}
#[derive(Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum Args {
    Read {
        file: String,
    },
    Sequence {
        files: [String; 2],
        seed: BTreeMap<String, String>,
        after: BTreeMap<String, String>,
    },
}
#[derive(Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum Outcome {
    Read { ok: Book },
    Sequence { ok: Sequence },
    Error { error: Error },
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Error {
    kind: String,
    workbook: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Book {
    date1904: bool,
    ssf: Vec<(u32, String)>,
    same_table: bool,
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
struct Bounds {
    s: Position,
    e: Position,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Position {
    r: u32,
    c: u32,
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
    value: Scalar,
    z: Option<()>,
    xf: Option<()>,
    w_after_read: Option<String>,
    text: Scalar,
    w_after_format: Option<String>,
}
#[derive(Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum Scalar {
    Number {
        #[serde(alias = "$number")]
        number: String,
    },
    Numeric(f64),
    Text(String),
    Boolean(bool),
}
impl Scalar {
    fn check(&self, value: &CellValue) {
        match (self, value) {
            (Self::Numeric(number), CellValue::Number(actual)) => {
                assert_eq!(number.to_bits(), actual.to_bits());
            }
            (Self::Number { number }, CellValue::Number(actual)) => {
                assert_eq!(actual.to_bits(), number.parse::<f64>().unwrap().to_bits());
            }
            (Self::Text(expected), CellValue::Text(actual)) => assert_eq!(actual, expected),
            (Self::Boolean(expected), CellValue::Boolean(actual)) => assert_eq!(actual, expected),
            _ => panic!("unexpected scalar classification"),
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Sequence {
    same_table: bool,
    initial_zero: String,
    custom_text: String,
    snapshot_format: String,
    live_after: String,
    unrelated_format: String,
    general_text: String,
}

pub use crate::workbook_fixture::bytes;

pub fn run(ids: &[&str]) {
    run_cases(ids, include_str!("../fixtures/ssf/legacy_workbooks.json"));
}

pub fn run_binary(ids: &[&str]) {
    run_cases(ids, include_str!("../fixtures/ssf/binary_workbooks.json"));
}

fn run_cases(ids: &[&str], data: &str) {
    let cases: Vec<Case> = serde_json::from_str(data).unwrap();
    let selected: Vec<_> = cases
        .into_iter()
        .filter(|c| ids.contains(&c.id.as_str()))
        .collect();
    assert_eq!(selected.len(), ids.len());
    for case in selected {
        match case.args {
            Args::Read { file } => {
                assert_eq!(case.op, "read");
                assert_eq!(case.id, file);
                check_read(&file, case.expected);
            }
            Args::Sequence { files, seed, after } => {
                assert_eq!(case.op, "read_sequence");
                let Outcome::Sequence { ok } = case.expected else {
                    panic!("expected sequence")
                };
                check_sequence(&files, seed, after, &ok);
            }
        }
    }
}

fn check_read(file: &str, expected: Outcome) {
    let mut ssf = Ssf::default();
    let alias = ssf.get_table();
    let result = ssf::read(&bytes(file), file, &mut ssf);
    match expected {
        Outcome::Read { ok } => {
            assert_eq!(Arc::ptr_eq(&alias, &ssf.get_table()), ok.same_table);
            check_book(result.unwrap(), &ok, &ssf);
        }
        Outcome::Error { error } => {
            let actual = result.unwrap_err();
            assert_eq!(error.kind, "reader");
            assert_eq!(error.workbook, file);
            assert!(matches!(actual, ssf::WorkbookError::Reader(_)));
            assert!(actual.to_string().contains(file));
        }
        Outcome::Sequence { .. } => panic!("unexpected sequence"),
    }
}

fn check_book(book: ssf::WorkBook, expected: &Book, ssf: &Ssf) {
    assert_eq!(book.date1904, expected.date1904);
    assert_eq!(
        book.ssf
            .iter()
            .map(|(k, v)| (*k, v.clone()))
            .collect::<Vec<_>>(),
        expected.ssf
    );
    assert_eq!(*ssf.get_table().read().unwrap(), book.ssf);
    assert_eq!(book.sheets.len(), expected.sheets.len());
    for ((name, sheet), expected) in book.sheets.into_iter().zip(&expected.sheets) {
        let sheet = sheet.unwrap();
        assert_eq!(name, expected.name);
        assert_eq!(sheet.range.map(ssf::encode_range), expected.range);
        assert_eq!(
            sheet.merges,
            expected
                .merges
                .iter()
                .map(Bounds::range)
                .collect::<Vec<_>>()
        );
        assert_eq!(sheet.cells.len(), expected.cells.len());
        for ((address, mut cell), expected) in sheet.cells.into_iter().zip(&expected.cells) {
            assert_eq!(address, expected.address);
            check_cell(&mut cell, expected, ssf);
        }
    }
}

fn check_cell(cell: &mut ssf::CellObject, expected: &Cell, ssf: &Ssf) {
    let kind = match cell.t.unwrap() {
        ssf::ExcelDataType::Number => "n",
        ssf::ExcelDataType::Text => "s",
        ssf::ExcelDataType::Boolean => "b",
        ssf::ExcelDataType::Error => "e",
        _ => panic!("unexpected cell classification"),
    };
    assert_eq!(kind, expected.r#type);
    expected.value.check(&cell.v);
    assert!(expected.z.is_none());
    assert!(expected.xf.is_none());
    assert!(cell.z.is_none());
    assert!(cell.xf.is_none());
    assert_eq!(cell.w, expected.w_after_read);
    expected.text.check(&ssf::format_cell(
        Some(cell),
        None,
        &CellFormatOptions::default(),
        ssf,
    ));
    assert_eq!(cell.w, expected.w_after_format);
    expected.value.check(&cell.v);
}

fn check_sequence(
    files: &[String; 2],
    seed: BTreeMap<String, String>,
    after: BTreeMap<String, String>,
    expected: &Sequence,
) {
    let mut ssf = Ssf::default();
    let alias = ssf.get_table();
    alias.write().unwrap().extend(
        seed.into_iter()
            .map(|(k, v)| (k.parse::<u32>().unwrap(), v)),
    );
    assert_eq!(alias.read().unwrap()[&0], expected.initial_zero);
    let book = ssf::read(&bytes(&files[0]), &files[0], &mut ssf).unwrap();
    assert_eq!(Arc::ptr_eq(&alias, &ssf.get_table()), expected.same_table);
    assert_eq!(
        book.sheets["Data"].as_ref().unwrap().cells["A1"]
            .w
            .as_deref(),
        Some(expected.custom_text.as_str())
    );
    alias.write().unwrap().extend(
        after
            .into_iter()
            .map(|(k, v)| (k.parse::<u32>().unwrap(), v)),
    );
    assert_eq!(book.ssf[&164], expected.snapshot_format);
    assert_eq!(ssf.get_table().read().unwrap()[&164], expected.live_after);
    assert_eq!(
        ssf.get_table().read().unwrap()[&999],
        expected.unrelated_format
    );
    let book = ssf::read(&bytes(&files[1]), &files[1], &mut ssf).unwrap();
    assert!(Arc::ptr_eq(&alias, &ssf.get_table()));
    assert_eq!(
        book.sheets["Old"].as_ref().unwrap().cells["A1"]
            .w
            .as_deref(),
        Some(expected.general_text.as_str())
    );
}

pub mod metadata;
