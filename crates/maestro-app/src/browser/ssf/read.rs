//! XLSX values projected through workbook metadata and the shared formatter.
use super::super::docx_preview::{OpenXmlPackage, Options, Relationship, resolve_path};
use super::{
    CellAddress, CellObject, CellValue, ExcelDataType, Range, Ssf, WorkBook, WorkSheet,
    WorkbookError, decode_cell, decode_range, encode_cell, format_cell::ERRORS, styles::Package,
};
use calamine::{DataRef, Reader, Xlsx, XlsxError};
use indexmap::IndexMap;
use std::{collections::BTreeMap, io::Cursor};
/// Read supplied XLSX bytes into owned sheets with formatted cell text.
///
/// # Errors
/// Returns contextual archive, missing-part, XML, native-reader or address failures.
pub fn read(data: &[u8], workbook_name: &str, ssf: &mut Ssf) -> Result<WorkBook, WorkbookError> {
    let package = Package {
        archive: OpenXmlPackage::open(data, Options::default())
            .map_err(|e| WorkbookError::Archive(format!("{workbook_name}: {e}")))?,
        name: workbook_name,
    };
    *ssf = Ssf::default();
    let text = package.text("[Content_Types].xml")?;
    let xml = package.xml(&text, "[Content_Types].xml")?;
    let parts: BTreeMap<_, _> = xml
        .elements(xml.root(), None)
        .filter_map(|node| Some((xml.attr(node, "ContentType")?, xml.attr(node, "PartName")?)))
        .collect();
    let workbook = parts
        .iter()
        .find(|(kind, _)| kind.ends_with("main+xml"))
        .map_or("xl/workbook.xml", |(_, part)| part.trim_start_matches('/'));
    let styles = parts
        .iter()
        .find(|(kind, _)| kind.ends_with("styles+xml"))
        .map(|(_, part)| part.trim_start_matches('/'));
    let xfs = package.styles(styles, ssf)?;
    let text = package.text(workbook)?;
    let xml = package.xml(&text, workbook)?;
    let date1904 = xml
        .element(xml.root(), "workbookPr")
        .and_then(|n| xml.bool_attr(n, "date1904", None))
        .unwrap_or(false);
    let rels = package
        .archive
        .relationships(Some(workbook))
        .map_err(|e| WorkbookError::Xml(format!("{workbook_name}: {workbook}: {e}")))?;
    let mut native = Xlsx::new(Cursor::new(data))
        .map_err(|e| WorkbookError::Reader(format!("{workbook_name}: {e}")))?;
    let mut sheets = IndexMap::new();
    if let Some(parent) = xml.element(xml.root(), "sheets") {
        for node in xml.elements(parent, Some("sheet")) {
            let name = xml.attr(node, "name").unwrap_or("");
            let target = xml
                .attr(node, "id")
                .and_then(|id| Relationship::find_target(rels.as_deref(), id))
                .ok_or_else(|| {
                    WorkbookError::MissingPart(format!("{workbook_name}: sheet {name}"))
                })?;
            let path = resolve_path(target, workbook);
            let values = values(&mut native, name, &package)?;
            sheets.insert(
                name.to_owned(),
                sheet(&package, &path, values, (&xfs, ssf, date1904))?,
            );
        }
    }
    Ok(WorkBook {
        sheets,
        date1904,
        ssf: ssf
            .table
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone(),
    })
}
/// Stream native cells, continuing after unsupported raw error labels only.
fn values(
    native: &mut Xlsx<Cursor<&[u8]>>,
    name: &str,
    package: &Package<'_>,
) -> Result<BTreeMap<(u32, u32), CellValue>, WorkbookError> {
    let mut reader = native
        .worksheet_cells_reader(name)
        .map_err(|e| WorkbookError::Reader(format!("{}: {name}: {e}", package.name)))?;
    let mut cells = BTreeMap::new();
    loop {
        match reader.next_cell() {
            Ok(Some(cell)) => {
                let value = match cell.get_value() {
                    DataRef::Float(n) => CellValue::Number(*n),
                    DataRef::Int(n) => {
                        CellValue::Number(num_traits::ToPrimitive::to_f64(n).unwrap_or(0.0))
                    }
                    DataRef::String(s) | DataRef::DateTimeIso(s) | DataRef::DurationIso(s) => {
                        CellValue::Text(s.clone())
                    }
                    DataRef::SharedString(s) => CellValue::Text((*s).to_owned()),
                    DataRef::Bool(b) => CellValue::Boolean(*b),
                    DataRef::DateTime(d) => CellValue::Number(d.as_f64()),
                    DataRef::Error(_) | DataRef::Empty => continue,
                };
                cells.insert(cell.get_position(), value);
            }
            Ok(None) => break,
            Err(XlsxError::CellError(_)) => {}
            Err(e) => {
                return Err(WorkbookError::Reader(format!(
                    "{}: {name}: {e}",
                    package.name
                )));
            }
        }
    }
    Ok(cells)
}
/// Overlay authored metadata without decoding native scalar values twice.
fn sheet(
    package: &Package<'_>,
    path: &str,
    mut values: BTreeMap<(u32, u32), CellValue>,
    format: (&[u32], &Ssf, bool),
) -> Result<WorkSheet, WorkbookError> {
    let text = package.text(path)?;
    let xml = package.xml(&text, path)?;
    let mut result = WorkSheet {
        cells: IndexMap::new(),
        range: None,
        merges: Vec::new(),
    };
    let mut row_index = 0;
    for row in xml
        .element(xml.root(), "sheetData")
        .into_iter()
        .flat_map(|data| xml.elements(data, Some("row")))
    {
        row_index = xml
            .attr(row, "r")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(row_index + 1);
        let mut position = CellAddress {
            r: row_index.saturating_sub(1),
            c: 0,
        };
        for node in xml.elements(row, Some("c")) {
            if let Some(address) = xml.attr(node, "r") {
                position = decode_cell(address)?;
            }
            let raw = xml.element(node, "v").map(|n| n.text_content());
            let value = values.remove(&(position.r, position.c));
            let value = if xml.attr(node, "t") == Some("d") {
                raw.as_deref()
                    .map(|s| super::datenum::datenum(s, format.2))
                    .transpose()?
                    .map(CellValue::Number)
            } else {
                value
            };
            let error = (xml.attr(node, "t") == Some("e")).then_some(raw).flatten();
            if let Some(cell) = cell(
                value,
                error,
                xml.attr(node, "s").and_then(|s| s.parse().ok()),
                format,
            ) {
                result.cells.insert(encode_cell(position), cell);
            }
            position.c = position.c.saturating_add(1);
        }
    }
    sheet_range(&xml, &mut result)?;
    Ok(result)
}
/// Retain declared bounds and merge order, falling back to observed cells.
fn sheet_range(
    xml: &super::super::docx_preview::XmlParser<'_>,
    result: &mut WorkSheet,
) -> Result<(), WorkbookError> {
    result.range = xml
        .element_attr(xml.root(), "dimension", "ref")
        .map(decode_range)
        .transpose()?;
    if result.cells.is_empty() {
        result.range = None;
    }
    if result.range.is_none() {
        for address in result.cells.keys() {
            let p = decode_cell(address)?;
            let range = result.range.get_or_insert(Range { s: p, e: p });
            range.s.r = range.s.r.min(p.r);
            range.s.c = range.s.c.min(p.c);
            range.e.r = range.e.r.max(p.r);
            range.e.c = range.e.c.max(p.c);
        }
    }
    if let Some(parent) = xml.element(xml.root(), "mergeCells") {
        result.merges = xml
            .elements(parent, Some("mergeCell"))
            .filter_map(|n| xml.attr(n, "ref"))
            .map(decode_range)
            .collect::<Result<_, _>>()?;
    }
    Ok(())
}
/// Preserve raw errors and leave failed individual formats uncached.
fn cell(
    value: Option<CellValue>,
    error: Option<String>,
    style: Option<usize>,
    format: (&[u32], &Ssf, bool),
) -> Option<CellObject> {
    if let Some(label) = error {
        let value = ERRORS
            .iter()
            .find(|(_, s)| *s == label)
            .map_or(CellValue::Missing, |(n, _)| {
                CellValue::Number(f64::from(*n))
            });
        return Some(CellObject {
            t: Some(ExcelDataType::Error),
            v: value,
            w: Some(label),
        });
    }
    let value = value?;
    let kind = match &value {
        CellValue::Number(_) => ExcelDataType::Number,
        CellValue::Text(_) => ExcelDataType::Text,
        CellValue::Boolean(_) => ExcelDataType::Boolean,
        CellValue::Missing | CellValue::Null => return None,
    };
    let id = style
        .and_then(|index| format.0.get(index))
        .copied()
        .unwrap_or(0);
    {
        let mut table = format
            .1
            .table
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        table.entry(id).or_insert_with(|| {
            let fallback = match id {
                32..=35 => 21,
                36 => 14,
                _ => super::default_map(id).unwrap_or(0),
            };
            super::default_str(id)
                .or_else(|| {
                    super::BUILTINS
                        .iter()
                        .find(|(n, _)| *n == fallback)
                        .map(|(_, s)| *s)
                })
                .unwrap_or("General")
                .to_owned()
        });
    }
    let shown = format.1.format(id, value.scalar(), format.2).ok();
    Some(CellObject {
        t: Some(kind),
        v: value,
        w: shown,
    })
}
