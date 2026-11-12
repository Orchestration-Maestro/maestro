//! Public spreadsheet state and coordinate behavior.
#![cfg(test)]
use maestro_office::spreadsheet::{
    CellFormatOptions, CellObject, CellValue, ExcelDataType, NumberFormatter, format_cell,
};

#[test]
fn formatter_reader_table_alias() {
    let formatter = NumberFormatter::default();
    let table = formatter.get_table();
    assert!(std::sync::Arc::ptr_eq(&table, &formatter.get_table()));
    table.write().unwrap().insert(0, "0.000".into());
    let mut cell = CellObject {
        f: None,
        array_formula: None,
        t: Some(ExcelDataType::Number),
        v: CellValue::Number(1.25),
        w: None,
        ..Default::default()
    };
    assert_eq!(
        format_cell(
            Some(&mut cell),
            None,
            &CellFormatOptions::default(),
            &formatter
        ),
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
fn formatter_default_cell_states() {
    let cases: Vec<DefaultCase> = serde_json::from_str(include_str!(
        "fixtures/spreadsheet_formatting/default-cells.json"
    ))
    .unwrap();
    for case in cases {
        let formatter = NumberFormatter::default();
        if let Some(code) = case.args.default_code {
            formatter.get_table().write().unwrap().insert(0, code);
        }
        let mut cell = case.args.cell.map(|c| CellObject {
            f: None,
            array_formula: None,
            t: c.t.as_deref().map(kind),
            v: c.v.value(),
            w: c.w,
            ..Default::default()
        });
        let text = format_cell(
            cell.as_mut(),
            None,
            &CellFormatOptions::default(),
            &formatter,
        );
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
fn formatter_address_roundtrip() {
    use maestro_office::spreadsheet::{
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
fn formatter_range_projection() {
    use maestro_office::spreadsheet::{CellAddress, Range, decode_range, encode_range};
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
fn formatter_retained_identity() {
    use maestro_office::spreadsheet::read;
    spreadsheet_format_support::run("formatter_retained_identity");
    let mut formatter = NumberFormatter::default();
    let old = formatter.get_table();
    old.write().unwrap().insert(164, "\"old\"0".into());
    let book = read(
        include_bytes!("fixtures/workbooks/custom.xlsx"),
        "custom.xlsx",
        &mut formatter,
    )
    .unwrap();
    let current = formatter.get_table();
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
    assert_eq!(book.formats.get(&164), None);
}

use crate::spreadsheet_format_support;

#[test]
fn formatter_general_dispatch() {
    spreadsheet_format_support::run("formatter_general_dispatch");
}

#[test]
fn formatter_section_layouts() {
    spreadsheet_format_support::run("formatter_section_layouts");
}

#[test]
fn formatter_nonfinite_and_failures() {
    spreadsheet_format_support::run("formatter_nonfinite_and_failures");
}

#[test]
fn formatter_conditional_selection() {
    spreadsheet_format_support::run("formatter_conditional_selection");
}

#[test]
fn formatter_condition_thresholds() {
    spreadsheet_format_support::run("formatter_condition_thresholds");
}

#[test]
fn formatter_empty_and_excess_sections() {
    spreadsheet_format_support::run("formatter_empty_and_excess_sections");
}

#[test]
fn formatter_numeric_ids() {
    spreadsheet_format_support::run("formatter_numeric_ids");
}

#[test]
fn formatter_quoted_delimiter_fix() {
    spreadsheet_format_support::run("formatter_quoted_delimiter_fix");
}

#[test]
fn formatter_quoted_controls() {
    spreadsheet_format_support::run("formatter_quoted_controls");
}

#[test]
fn formatter_mixed_text_numeric_sections() {
    spreadsheet_format_support::run("formatter_mixed_text_numeric_sections");
}

#[test]
fn formatter_option_consumption() {
    spreadsheet_format_support::run("formatter_option_consumption");
}

#[test]
fn formatter_date_serial_conversion() {
    spreadsheet_format_support::run("formatter_date_serial_conversion");
}

#[test]
fn formatter_date_general() {
    spreadsheet_format_support::run("formatter_date_general");
}

#[test]
fn formatter_date_epoch_routes() {
    spreadsheet_format_support::run("formatter_date_epoch_routes");
}

#[test]
fn formatter_landed_corrections() {
    spreadsheet_format_support::run("formatter_landed_corrections");
}

#[test]
fn formatter_load_auto() {
    spreadsheet_format_support::run("formatter_load_auto");
}

#[test]
fn formatter_load_explicit() {
    spreadsheet_format_support::run("formatter_load_explicit");
}

#[test]
fn formatter_load_full() {
    spreadsheet_format_support::run("formatter_load_full");
}

#[test]
fn formatter_load_table_bounds() {
    spreadsheet_format_support::run("formatter_load_table_bounds");
}

#[test]
fn formatter_table_alias() {
    spreadsheet_format_support::run("formatter_table_alias");
}

#[test]
fn formatter_cell_cached() {
    spreadsheet_format_support::run("formatter_cell_cached");
}

#[test]
fn formatter_cell_empty() {
    spreadsheet_format_support::run("formatter_cell_empty");
    let mut cell = CellObject::default();
    assert_eq!(cell.v, CellValue::Missing);
    assert_eq!(cell.t, None);
    assert_eq!(cell.w, None);
    assert_eq!(cell.z, None);
    assert_eq!(cell.xf, None);
    assert_eq!(
        format_cell(
            Some(&mut cell),
            None,
            &CellFormatOptions::default(),
            &NumberFormatter::default()
        ),
        CellValue::Text(String::new()),
    );
}

#[test]
fn formatter_cell_patterns() {
    spreadsheet_format_support::run("formatter_cell_patterns");
}

#[test]
fn formatter_cell_double_failure_preserves_raw_scalars() {
    use maestro_office::spreadsheet::{NumberFormat, Xf};

    let formatter = NumberFormatter::default();
    let _ = formatter.load("\"bad", Some(164));
    let actual: Vec<_> = [
        CellValue::Boolean(true),
        CellValue::Boolean(false),
        CellValue::Null,
        CellValue::Missing,
    ]
    .into_iter()
    .map(|value| {
        let mut cell = CellObject {
            t: Some(ExcelDataType::Boolean),
            v: value.clone(),
            z: Some(NumberFormat::Code("\"bad".into())),
            xf: Some(Xf { num_fmt_id: 164 }),
            ..CellObject::default()
        };
        let text = format_cell(
            Some(&mut cell),
            None,
            &CellFormatOptions::default(),
            &formatter,
        );
        assert_eq!(cell.v, value);
        assert_eq!(cell.w, None);
        text
    })
    .collect();
    assert_eq!(
        actual,
        ["true", "false", "null", "undefined"].map(|text| CellValue::Text(text.into()))
    );
}

#[test]
fn formatter_cell_single_failure_uses_xf_format() {
    use maestro_office::spreadsheet::{NumberFormat, Xf};

    let formatter = NumberFormatter::default();
    let _ = formatter.load("0.000", Some(164));
    let mut cell = CellObject {
        t: Some(ExcelDataType::Number),
        v: CellValue::Number(1.25),
        z: Some(NumberFormat::Code("\"bad".into())),
        xf: Some(Xf { num_fmt_id: 164 }),
        ..CellObject::default()
    };
    assert_eq!(
        format_cell(
            Some(&mut cell),
            None,
            &CellFormatOptions::default(),
            &formatter
        ),
        CellValue::Text("1.250".into())
    );
    assert_eq!(cell.w.as_deref(), Some("1.250"));
}

#[test]
fn formatter_cell_override() {
    spreadsheet_format_support::run("formatter_cell_override");
}

#[test]
fn formatter_cell_dates() {
    use maestro_office::spreadsheet::NumberFormat;
    spreadsheet_format_support::run("formatter_cell_dates");
    let mut cell = CellObject {
        t: Some(ExcelDataType::Date),
        v: CellValue::Date(
            chrono::DateTime::parse_from_rfc3339("2023-01-01T00:00:00Z")
                .unwrap()
                .to_utc(),
        ),
        z: Some(NumberFormat::Code("yyyy".into())),
        ..CellObject::default()
    };
    let override_format = NumberFormat::Code("mm".into());
    let options = CellFormatOptions {
        date_nf: Some(&override_format),
    };
    assert_eq!(
        format_cell(Some(&mut cell), None, &options, &NumberFormatter::default()),
        CellValue::Text("2023".into())
    );
    assert_eq!(cell.z, Some(NumberFormat::Code("yyyy".into())));
}

#[test]
fn formatter_cell_error_text() {
    spreadsheet_format_support::run("formatter_cell_error_text");
}
