//! Shared worksheet table projection with literal attribute boundaries.
use super::{
    CellAddress, CellFormatOptions, CellObject, CellValue, ExcelDataType, NumberFormat, Ssf,
    WorkSheet, encode_cell, format_cell,
};
use crate::browser::executable;
use maestro_text::markup::{EscapeMode, escape};
use std::fmt::Write as _;
/// Project the declared worksheet range as a table, retaining formatter caches.
#[must_use]
pub fn sheet_to_html(worksheet: &mut WorkSheet, id: &str, ssf: &Ssf) -> String {
    let mut out = String::from("<table");
    if !id.is_empty() {
        attribute(&mut out, "id", id);
    }
    out.push('>');
    if let Some(range) = worksheet.range {
        for r in range.s.r..=range.e.r {
            out.push_str("<tr>");
            for c in range.s.c..=range.e.c {
                table_cell(worksheet, CellAddress { r, c }, (id, ssf), &mut out);
            }
            out.push_str("</tr>");
        }
    }
    out.push_str("</table>");
    out
}
/// Emit a merge anchor or ordinary cell at its absolute address.
fn table_cell(
    worksheet: &mut WorkSheet,
    position: CellAddress,
    context: (&str, &Ssf),
    out: &mut String,
) {
    let (id, ssf) = context;
    let merge = worksheet.merges.iter().find(|m| m.contains(position));
    if merge.is_some_and(|m| m.s != position) {
        return;
    }
    let address = encode_cell(position);
    out.push_str("<td");
    if let Some(m) = merge {
        if m.e.r > m.s.r {
            attribute(out, "rowspan", &(m.e.r - m.s.r + 1).to_string());
        }
        if m.e.c > m.s.c {
            attribute(out, "colspan", &(m.e.c - m.s.c + 1).to_string());
        }
    }
    let body = worksheet
        .cells
        .get_mut(&address)
        .map_or_else(String::new, |cell| project(cell, out, ssf));
    attribute(
        out,
        "id",
        &format!("{}-{address}", if id.is_empty() { "sjs" } else { id }),
    );
    if body.starts_with(maestro_text::is_whitespace_scalar)
        || body.ends_with(maestro_text::is_whitespace_scalar)
        || body.contains('\n')
    {
        out.push_str(" xml:space=\"preserve\"");
    }
    out.push('>');
    out.push_str(&body);
    out.push_str("</td>");
}
/// Serialize a literal attribute through the common attribute escaper.
fn attribute(out: &mut String, key: &str, value: &str) {
    let _ = write!(
        out,
        " {key}=\"{}\"",
        escape(value, EscapeMode::XmlAttribute)
    );
}
/// Select content independently of raw attributes and cache ownership.
fn project(cell: &mut CellObject, out: &mut String, ssf: &Ssf) -> String {
    if cell.t == Some(ExcelDataType::Number)
        && let CellValue::Number(n) = cell.v
        && !n.is_finite()
    {
        let (code, label) = if n.is_nan() {
            (36., "#NUM!")
        } else {
            (7., "#DIV/0!")
        };
        return project(
            &mut CellObject {
                t: Some(ExcelDataType::Error),
                v: CellValue::Number(code),
                w: Some(label.into()),
                ..Default::default()
            },
            out,
            ssf,
        );
    }
    let present = !matches!(cell.v, CellValue::Missing | CellValue::Null);
    let body = content(cell, ssf);
    attribute(out, "data-t", cell_type(cell.t));
    if present {
        let _ = write!(
            out,
            " data-v=\"{}\"",
            escape(
                &super::format_cell::raw_text(&cell.v),
                EscapeMode::SpreadsheetHtml
            )
        );
    }
    if let Some(z) = &cell.z {
        match z {
            NumberFormat::Code(code) => attribute(out, "data-z", code),
            NumberFormat::Id(id) => attribute(out, "data-z", &id.to_string()),
        }
    }
    if let Some(link) = &cell.l
        && !link.target.is_empty()
        && !link.target.starts_with('#')
        && !executable(&link.target)
    {
        format!(
            "<a href=\"{}\">{body}</a>",
            escape(&link.target, EscapeMode::SpreadsheetHtml)
        )
    } else {
        body
    }
}
/// Preserve the authored classification independently of its scalar value.
fn cell_type(kind: Option<ExcelDataType>) -> &'static str {
    match kind {
        Some(ExcelDataType::Number) => "n",
        Some(ExcelDataType::Text) => "s",
        Some(ExcelDataType::Boolean) => "b",
        Some(ExcelDataType::Error) => "e",
        Some(ExcelDataType::Date) => "d",
        Some(ExcelDataType::Stub) | None => "z",
    }
}
/// Read generated markup before escaped formatted text, without caching errors.
fn content(cell: &mut CellObject, ssf: &Ssf) -> String {
    if matches!(cell.v, CellValue::Missing | CellValue::Null) {
        return String::new();
    }
    if let Some(h) = cell.h.as_ref().filter(|h| !h.is_empty()) {
        return h.clone();
    }
    if cell.w.is_none() {
        let _ = format_cell(Some(cell), None, &CellFormatOptions::default(), ssf);
    }
    escape(cell.w.as_deref().unwrap_or(""), EscapeMode::SpreadsheetHtml)
}
