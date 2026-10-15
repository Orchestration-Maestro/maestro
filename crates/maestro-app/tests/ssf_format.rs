//! Public spreadsheet state and coordinate behavior.
#![cfg(test)]
use maestro_app::browser::ssf::{CellObject, CellValue, ExcelDataType, Ssf, format_cell};

#[test]
fn ssf_reader_table_alias() {
    let ssf = Ssf::default();
    let table = ssf.get_table();
    assert!(std::sync::Arc::ptr_eq(&table, &ssf.get_table()));
    table.write().unwrap().insert(0, "0.000".into());
    let mut cell = CellObject {
        t: Some(ExcelDataType::Number),
        v: CellValue::Number(1.25),
        w: None,
    };
    assert_eq!(
        format_cell(Some(&mut cell), &ssf),
        CellValue::Text("1.250".into())
    );
    assert_eq!(cell.w.as_deref(), Some("1.250"));
    assert_eq!(cell.v, CellValue::Number(1.25));
}

/// Scalar encoding in the independently recorded corpus.
#[derive(serde::Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum Scalar {
    Number { number: String },
    Empty { empty: String },
    Boolean { boolean: bool },
    Text { text: String },
}
impl Scalar {
    fn value(&self) -> CellValue {
        match self {
            Self::Number { number } => CellValue::Number(number.parse().unwrap()),
            Self::Empty { empty } if empty == "null" => CellValue::Null,
            Self::Empty { empty } => {
                assert_eq!(empty, "missing");
                CellValue::Missing
            }
            Self::Boolean { boolean } => CellValue::Boolean(*boolean),
            Self::Text { text } => CellValue::Text(text.clone()),
        }
    }
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct InputCell {
    t: Option<String>,
    v: Scalar,
    w: Option<String>,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct DefaultArgs {
    cell: Option<InputCell>,
    default_code: Option<String>,
}
#[derive(serde::Deserialize)]
#[serde(untagged)]
enum DefaultText {
    Text(String),
    Number(f64),
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct DefaultExpected {
    text: DefaultText,
    w: Option<String>,
    v_after: Option<Scalar>,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct DefaultCase {
    args: DefaultArgs,
    expected: DefaultExpected,
}
fn kind(t: &str) -> ExcelDataType {
    match t {
        "n" => ExcelDataType::Number,
        "s" => ExcelDataType::Text,
        "b" => ExcelDataType::Boolean,
        "e" => ExcelDataType::Error,
        "z" => ExcelDataType::Stub,
        _ => panic!("unknown type"),
    }
}
fn assert_value(actual: &CellValue, expected: &CellValue) {
    match (actual, expected) {
        (CellValue::Number(a), CellValue::Number(b)) => assert_eq!(a.to_bits(), b.to_bits()),
        _ => assert_eq!(actual, expected),
    }
}
#[test]
fn ssf_default_cell_states() {
    let cases: Vec<DefaultCase> =
        serde_json::from_str(include_str!("fixtures/ssf/default-cells.json")).unwrap();
    for case in cases {
        let ssf = Ssf::default();
        if let Some(code) = case.args.default_code {
            ssf.get_table().write().unwrap().insert(0, code);
        }
        let mut cell = case.args.cell.map(|c| CellObject {
            t: c.t.as_deref().map(kind),
            v: c.v.value(),
            w: c.w,
        });
        let text = format_cell(cell.as_mut(), &ssf);
        let expected = match case.expected.text {
            DefaultText::Text(s) => CellValue::Text(s),
            DefaultText::Number(n) => CellValue::Number(n),
        };
        assert_value(&text, &expected);
        assert_eq!(
            cell.as_ref().and_then(|c| c.w.as_ref()),
            case.expected.w.as_ref()
        );
        match (cell, case.expected.v_after) {
            (Some(c), Some(v)) => assert_value(&c.v, &v.value()),
            (None, None) => {}
            _ => panic!("cell presence changed"),
        }
    }
}

#[test]
fn ssf_address_roundtrip() {
    use maestro_app::browser::ssf::{
        CellAddress, decode_cell, encode_cell, encode_col, encode_row,
    };
    for (r, c, text, row, col) in [
        (0, 0, "A1", "1", "A"),
        (0, 25, "Z1", "1", "Z"),
        (1, 26, "AA2", "2", "AA"),
        (9, 51, "AZ10", "10", "AZ"),
        (1_048_575, 16383, "XFD1048576", "1048576", "XFD"),
    ] {
        let cell = CellAddress { r, c };
        assert_eq!(encode_cell(cell), text);
        assert_eq!(decode_cell(text).unwrap(), cell);
        assert_eq!(encode_row(r), row);
        assert_eq!(encode_col(c), col);
    }
}

#[test]
fn ssf_range_projection() {
    use maestro_app::browser::ssf::{CellAddress, Range, decode_range, encode_range};
    for (input, start, end, output) in [
        ("A1", (0, 0), (0, 0), "A1"),
        ("Z10:AA11", (9, 25), (10, 26), "Z10:AA11"),
        ("A1:XFD1048576", (0, 0), (1_048_575, 16383), "A1:XFD1048576"),
        ("$A$1:$B$2", (0, 0), (1, 1), "A1:B2"),
        ("D9:D9", (8, 3), (8, 3), "D9"),
    ] {
        let expected = Range {
            s: CellAddress {
                r: start.0,
                c: start.1,
            },
            e: CellAddress { r: end.0, c: end.1 },
        };
        let actual = decode_range(input).unwrap();
        assert_eq!(actual, expected);
        assert_eq!(encode_range(actual), output);
    }
}

#[test]
fn ssf_retained_identity() {
    use maestro_app::browser::ssf::read;
    let mut ssf = Ssf::default();
    let old = ssf.get_table();
    old.write().unwrap().insert(164, "\"old\"0".into());
    let book = read(
        include_bytes!("fixtures/workbooks/custom.xlsx"),
        "custom.xlsx",
        &mut ssf,
    )
    .unwrap();
    let current = ssf.get_table();
    assert!(!std::sync::Arc::ptr_eq(&old, &current));
    current.write().unwrap().insert(164, "\"new\"0".into());
    assert_eq!(
        old.read().unwrap().get(&164).map(String::as_str),
        Some("\"old\"0")
    );
    assert_eq!(
        current.read().unwrap().get(&164).map(String::as_str),
        Some("\"new\"0")
    );
    assert_eq!(book.ssf.get(&164), None);
}
