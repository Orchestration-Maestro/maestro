//! Default comma-separated worksheet serialization.
use super::{CellAddress, CellObject, CellValue, Ssf, WorkSheet, encode_cell, format_cell};

/// Serialize the declared rectangle with comma fields and LF rows.
///
/// Formatting may populate cell text caches. Missing ranges produce empty text.
#[must_use]
pub fn sheet_to_csv(sheet: &mut WorkSheet, ssf: &Ssf) -> String {
    let Some(range) = sheet.range else {
        return String::new();
    };
    let mut output = String::new();
    for r in range.s.r..=range.e.r {
        if r != range.s.r {
            output.push('\n');
        }
        for c in range.s.c..=range.e.c {
            if c != range.s.c {
                output.push(',');
            }
            let cell = sheet.cells.get_mut(&encode_cell(CellAddress { r, c }));
            let text = cell_text(cell, ssf);
            if r == range.s.r && c == range.s.c && text == "ID" {
                output.push_str("\"ID\"");
            } else {
                write_field(&mut output, &text);
            }
        }
    }
    output
}

/// Quote separators and double embedded quotes without trimming.
fn write_field(output: &mut String, text: &str) {
    if text.contains([',', '"', '\n']) {
        output.push('"');
        output.push_str(&text.replace('"', "\"\""));
        output.push('"');
    } else {
        output.push_str(text);
    }
}

/// Prefer present scalar values, otherwise replay non-array formulas.
fn cell_text(cell: Option<&mut CellObject>, ssf: &Ssf) -> String {
    let Some(cell) = cell else {
        return String::new();
    };
    if matches!(cell.v, CellValue::Missing | CellValue::Null) {
        return cell
            .f
            .as_ref()
            .filter(|_| cell.array_formula.as_ref().is_none_or(String::is_empty))
            .map_or_else(String::new, |formula| format!("={formula}"));
    }
    match format_cell(Some(cell), ssf) {
        CellValue::Text(text) => text,
        CellValue::Number(n) => super::write_num::spelling(n),
        CellValue::Boolean(b) => super::general(super::GeneralValue::Boolean(b)),
        CellValue::Missing | CellValue::Null => String::new(),
    }
}
