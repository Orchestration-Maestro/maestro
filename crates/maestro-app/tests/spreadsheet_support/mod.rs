//! Strict independent spreadsheet corpus consumers.
#![cfg(test)]
use maestro_app::browser::ssf::*;
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Table {
    name: String,
    input: Input,
    expected: Expected,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    worksheet: BTreeMap<String, Value>,
    id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    table: String,
    cache: BTreeMap<String, Option<String>>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Cell {
    t: Option<String>,
    #[serde(default)]
    v: Value,
    w: Option<String>,
    h: Option<String>,
    z: Option<String>,
    l: Option<Link>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Link {
    #[serde(rename = "Target")]
    target: Option<String>,
}
fn cell(value: Value) -> CellObject {
    let missing = value.get("v").is_none();
    let c: Cell = serde_json::from_value(value).unwrap();
    CellObject {
        t: c.t.map(|s| match s.as_str() {
            "n" => ExcelDataType::Number,
            "s" => ExcelDataType::Text,
            "b" => ExcelDataType::Boolean,
            "e" => ExcelDataType::Error,
            "z" => ExcelDataType::Stub,
            _ => panic!("unknown cell type"),
        }),
        v: if missing {
            CellValue::Missing
        } else {
            match c.v {
                Value::Null => CellValue::Null,
                Value::String(s) => CellValue::Text(s),
                Value::Bool(b) => CellValue::Boolean(b),
                Value::Number(n) => CellValue::Number(n.as_f64().unwrap()),
                Value::Object(mut o) => {
                    let n = o
                        .shift_remove("$number")
                        .unwrap()
                        .as_str()
                        .unwrap()
                        .parse()
                        .unwrap();
                    assert!(o.is_empty());
                    CellValue::Number(n)
                }
                Value::Array(_) => panic!("non scalar"),
            }
        },
        w: c.w,
        h: c.h,
        z: c.z.map(NumberFormat::Code),
        l: c.l
            .and_then(|l| l.target)
            .map(|target| Hyperlink { target }),
        ..Default::default()
    }
}
fn worksheet(mut input: BTreeMap<String, Value>) -> WorkSheet {
    let range = input
        .remove("!ref")
        .map(|v| decode_range(v.as_str().unwrap()).unwrap());
    let merges = input
        .remove("!merges")
        .map(|v| {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Point {
                r: u32,
                c: u32,
            }
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Merge {
                s: Point,
                e: Point,
            }
            serde_json::from_value::<Vec<Merge>>(v)
                .unwrap()
                .into_iter()
                .map(|m| Range {
                    s: CellAddress { r: m.s.r, c: m.s.c },
                    e: CellAddress { r: m.e.r, c: m.e.c },
                })
                .collect()
        })
        .unwrap_or_default();
    WorkSheet {
        range,
        merges,
        cells: input
            .into_iter()
            .map(|(k, v)| {
                decode_cell(&k).unwrap();
                (k, cell(v))
            })
            .collect(),
    }
}
/// Assert all selected distinct table inputs and post-projection caches.
pub fn tables(names: &[&str]) {
    let rows: Vec<Table> =
        serde_json::from_str(include_str!("../fixtures/spreadsheet_html/html.json")).unwrap();
    let mut found = Vec::new();
    for row in rows
        .into_iter()
        .filter(|r| names.contains(&r.name.as_str()))
    {
        found.push(row.name.clone());
        let mut sheet = worksheet(row.input.worksheet);
        let raw: Vec<_> = sheet.cells.values().map(|c| c.v.clone()).collect();
        assert_eq!(
            sheet_to_html(&mut sheet, &row.input.id, &Ssf::default()),
            row.expected.table,
            "{}",
            row.name
        );
        assert_eq!(
            sheet
                .cells
                .iter()
                .map(|(k, c)| (k.clone(), c.w.clone()))
                .collect::<BTreeMap<_, _>>(),
            row.expected.cache,
            "{}",
            row.name
        );
        for (cell, before) in sheet.cells.values().zip(raw) {
            match (&cell.v, before) {
                (CellValue::Number(a), CellValue::Number(b)) => {
                    assert_eq!(a.to_bits(), b.to_bits());
                }
                (a, b) => assert_eq!(*a, b),
            }
        }
    }
    assert_eq!(found.len(), names.len(), "missing table input");
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Workbook {
    file: String,
    expected: Vec<Sheet>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Sheet {
    name: String,
    cells: BTreeMap<String, Value>,
    table: String,
}
/// Read real archived workbooks and compare every retained field and table.
pub fn workbooks(names: &[&str]) {
    let rows: Vec<Workbook> =
        serde_json::from_str(include_str!("../fixtures/spreadsheet_html/workbooks.json")).unwrap();
    let mut found = 0;
    for row in rows
        .into_iter()
        .filter(|r| names.contains(&r.file.as_str()))
    {
        found += 1;
        let bytes = std::fs::read(format!(
            "{}/tests/fixtures/spreadsheet_html/{}",
            env!("CARGO_MANIFEST_DIR"),
            row.file
        ))
        .unwrap();
        let mut ssf = Ssf::default();
        let mut workbook = read(&bytes, &row.file, &mut ssf).unwrap();
        assert_eq!(
            workbook.sheets.keys().collect::<Vec<_>>(),
            row.expected.iter().map(|s| &s.name).collect::<Vec<_>>()
        );
        for expected in row.expected {
            let actual = workbook
                .sheets
                .get_mut(&expected.name)
                .unwrap()
                .as_mut()
                .unwrap();
            let sheet = worksheet(expected.cells);
            assert_eq!(actual.range, sheet.range, "{}", row.file);
            assert_eq!(actual.merges, sheet.merges, "{}", row.file);
            assert_eq!(actual.cells, sheet.cells, "{}", row.file);
            assert_eq!(
                sheet_to_html(actual, &format!("sheet-{}", expected.name), &ssf),
                expected.table,
                "{}",
                row.file
            );
        }
    }
    assert_eq!(found, names.len());
}
