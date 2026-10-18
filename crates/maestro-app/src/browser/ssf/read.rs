//! Modern and legacy workbook values projected through metadata and the shared formatter.
use super::super::docx_preview::{
    OpenXmlPackage, Options, Relationship, XmlElement, XmlParser, float_prefix, resolve_path,
};
use super::rich_text::{SharedString, shared_string};
use super::{
    CellAddress, CellObject, CellValue, ExcelDataType, Range, Ssf, WorkBook, WorkSheet,
    WorkbookError, decode_cell, decode_range, encode_cell, format_cell::ERRORS, styles::Package,
};
mod blob;
mod cfb;
mod formulas;
mod xls;
use indexmap::IndexMap;
use std::collections::BTreeMap;
/// Read supplied XLSX or compound-file XLS bytes into owned sheets with formatted cell text.
/// Legacy sheets include declared blank bounds and load repaired formats into the live table.
/// Both formats retain formulas, array ranges and hyperlink targets beside cached values.
/// Binary rich strings retain decoded plain text without generated run markup.
/// XLSX retains sheet-local errors beside readable sheets, including valid blank sheets.
/// A workbook with no declared sheets succeeds with an empty collection.
///
/// # Errors
/// Returns contextual archive, shared metadata, container, record or address failures.
/// When every declared XLSX sheet fails, returns the first sheet's original error.
pub fn read(data: &[u8], workbook_name: &str, ssf: &mut Ssf) -> Result<WorkBook, WorkbookError> {
    if data.starts_with(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]) {
        return xls::read(data, workbook_name, ssf);
    }
    read_xlsx(data, workbook_name, ssf)
}
/// Read indexed modern workbook parts, replacing the active table after archive opening.
fn read_xlsx(data: &[u8], workbook_name: &str, ssf: &mut Ssf) -> Result<WorkBook, WorkbookError> {
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
    let part = |suffix: &str| {
        parts
            .iter()
            .find(|(kind, _)| kind.ends_with(suffix))
            .map(|(_, part)| part.trim_start_matches('/'))
    };
    let workbook = part("main+xml").unwrap_or("xl/workbook.xml");
    let xfs = package.styles(part("styles+xml"), ssf)?;
    let shared = super::rich_text::shared(&package, part("sharedStrings+xml"))?;
    let text = package.text(workbook)?;
    let xml = package.xml(&text, workbook)?;
    let date1904 = xml
        .element(xml.root(), "workbookPr")
        .and_then(|n| xml.bool_attr(n, "date1904", None))
        .unwrap_or(false);
    let sheets = read_sheets(&package, &xml, workbook, (&xfs, ssf, date1904), &shared)?;
    Ok(WorkBook {
        sheets,
        date1904,
        ssf: ssf
            .table
            .read()
            .map_or_else(|error| error.into_inner().clone(), |table| table.clone()),
    })
}
/// Resolve worksheet-local results without discarding readable siblings.
/// An unresolved identity switches every sheet to ordered positional paths.
fn read_sheets(
    package: &Package<'_>,
    xml: &XmlParser<'_>,
    workbook: &str,
    format: (&[u32], &Ssf, bool),
    shared: &[SharedString],
) -> Result<IndexMap<String, Result<WorkSheet, WorkbookError>>, WorkbookError> {
    let rels = package
        .archive
        .relationships(Some(workbook))
        .map_err(|e| WorkbookError::Xml(format!("{}: {workbook}: {e}", package.name)))?;
    let targets = sheet_targets(rels.as_deref(), package.name)?;
    let names: Vec<_> = xml
        .element(xml.root(), "sheets")
        .into_iter()
        .flat_map(|parent| xml.elements(parent, Some("sheet")))
        .map(|node| {
            (
                xml.attr(node, "name").unwrap_or(""),
                xml.attr(node, "id").filter(|id| !id.is_empty()),
            )
        })
        .collect();
    let fallback = names
        .iter()
        .any(|(_, id)| id.is_none_or(|id| !targets.contains_key(id)));
    let nmode = package.archive.contains("xl/worksheets/sheet.xml");
    let mut sheets = IndexMap::new();
    for (i, (name, id)) in names.into_iter().enumerate() {
        let path = if fallback {
            let suffix = if nmode && i == 0 {
                String::new()
            } else {
                (i + usize::from(!nmode)).to_string()
            };
            Ok(format!("xl/worksheets/sheet{suffix}.xml"))
        } else {
            id.and_then(|id| targets.get(id))
                .filter(|target| !target.is_empty())
                .map(|target| resolve_path(target, workbook))
                .ok_or_else(|| {
                    WorkbookError::MissingPart(format!("{}: sheet {name}", package.name))
                })
        };
        let result = path.and_then(|path| sheet(package, &path, format, shared));
        sheets.insert(name.to_owned(), result);
    }
    if sheets.values().all(Result::is_err)
        && let Some((_, Err(error))) = sheets.shift_remove_index(0)
    {
        return Err(error);
    }
    Ok(sheets)
}
/// Index workbook targets once, keeping the last authored relationship for each identity.
fn sheet_targets<'a>(
    rels: Option<&'a [Relationship]>,
    workbook_name: &str,
) -> Result<IndexMap<&'a str, &'a str>, WorkbookError> {
    let mut targets = IndexMap::new();
    for rel in rels.into_iter().flatten() {
        let target = rel.target.as_deref().ok_or_else(|| {
            WorkbookError::MissingPart(format!("{workbook_name}: relationship target"))
        })?;
        if let Some(id) = rel.id.as_deref() {
            targets.insert(id, target);
        }
    }
    Ok(targets)
}
/// Decode stored scalars and authored metadata in the same XML pass.
fn sheet(
    package: &Package<'_>,
    path: &str,
    format: (&[u32], &Ssf, bool),
    shared: &[SharedString],
) -> Result<WorkSheet, WorkbookError> {
    let text = package.text(path)?;
    let xml = package.xml(&text, path)?;
    let mut result = WorkSheet {
        cells: IndexMap::new(),
        range: None,
        merges: Vec::new(),
    };
    let mut arrays = Vec::new();
    let mut shared_formulas = formulas::Shared::new();
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
            let formula = formulas::formula(&xml, node, position, &mut shared_formulas)?;
            let array = formulas::array_range(&xml, node, position, &mut arrays)?;
            let value = scalar(&xml, node, raw.as_deref(), shared)
                .map_err(|e| WorkbookError::Reader(format!("{}: {path}: {e}", package.name)))?;
            let formulas = (formula, array);
            let value = raw_value(
                xml.attr(node, "t"),
                raw.as_deref(),
                value,
                &formulas,
                format.2,
            )?;
            let error = (xml.attr(node, "t") == Some("e")).then_some(raw).flatten();
            if let Some(mut cell) = cell(
                value,
                error,
                xml.attr(node, "s").and_then(|s| s.parse().ok()),
                format,
                formulas,
            ) {
                super::rich_text::adopt(&xml, node, &mut cell, shared);
                result.cells.insert(encode_cell(position), cell);
            }
            position.c = position.c.saturating_add(1);
        }
    }
    sheet_range(&xml, &mut result)?;
    super::hyperlinks::adopt(package, path, &xml, &mut result)?;
    Ok(result)
}
/// Decode stored scalar values before applying formula and date metadata.
fn scalar(
    xml: &XmlParser<'_>,
    node: XmlElement<'_, '_>,
    raw: Option<&str>,
    shared: &[SharedString],
) -> Result<Option<CellValue>, String> {
    let raw = raw.filter(|raw| !raw.is_empty());
    Ok(match xml.attr(node, "t") {
        None | Some("n") => raw.map(|raw| CellValue::Number(float_prefix(raw))),
        Some("s") => raw
            .map(|raw| {
                shared_string(raw, shared)
                    .map(|string| CellValue::Text(string.text.clone()))
                    .ok_or_else(|| format!("shared string {raw}"))
            })
            .transpose()?,
        Some("b") => Some(CellValue::Boolean(matches!(raw, Some("1" | "true")))),
        Some("inlineStr") => Some(CellValue::Text(
            xml.element(node, "is")
                .map_or_else(String::new, |is| super::rich_text::text(xml, is)),
        )),
        _ => None,
    })
}
/// Apply authored strings, dates and missing formula caches to stored scalars.
fn raw_value(
    kind: Option<&str>,
    raw: Option<&str>,
    value: Option<CellValue>,
    formulas: &(Option<String>, Option<String>),
    date1904: bool,
) -> Result<Option<CellValue>, WorkbookError> {
    Ok(match kind {
        Some("str") => Some(CellValue::Text(
            raw.map(super::unescapexml::unescapexml).unwrap_or_default(),
        )),
        Some("d") => raw
            .map(|s| super::datenum::datenum(s, date1904))
            .transpose()?
            .map(CellValue::Number),
        None | Some("n")
            if raw.is_none_or(str::is_empty) && (formulas.0.is_some() || formulas.1.is_some()) =>
        {
            Some(CellValue::Missing)
        }
        _ => value,
    })
}
/// Retain declared bounds and merge order, falling back to observed cells.
fn sheet_range(
    xml: &super::super::docx_preview::XmlParser<'_>,
    result: &mut WorkSheet,
) -> Result<(), WorkbookError> {
    result.range = xml
        .element_attr(xml.root(), "dimension", "ref")
        .filter(|s| s.contains(':') || !result.cells.is_empty())
        .map(decode_range)
        .transpose()?;
    if result.range.is_none() {
        for address in result.cells.keys() {
            let p = decode_cell(address)?;
            let range = result.range.get_or_insert(Range { s: p, e: p });
            grow_range(range, p);
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
/// Include one observed cell in an inclusive rectangle.
fn grow_range(range: &mut Range, position: CellAddress) {
    range.s.r = range.s.r.min(position.r);
    range.s.c = range.s.c.min(position.c);
    range.e.r = range.e.r.max(position.r);
    range.e.c = range.e.c.max(position.c);
}
/// Preserve raw errors and leave failed individual formats uncached.
fn cell(
    value: Option<CellValue>,
    error: Option<String>,
    style: Option<usize>,
    format: (&[u32], &Ssf, bool),
    formulas: (Option<String>, Option<String>),
) -> Option<CellObject> {
    let value = value.map(|value| match value {
        CellValue::Text(text) => CellValue::Text(text.replace("\r\n", "\n")),
        value => value,
    });
    let mut cell = basic_cell(value, error, formulas)?;
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
    if matches!(cell.v, CellValue::Missing) {
        return Some(cell);
    }
    cell.w = cell.w.or_else(|| {
        format
            .1
            .format(
                &super::NumberFormat::Id(id),
                &cell.v,
                &super::FormatOptions {
                    date1904: format.2,
                    ..Default::default()
                },
            )
            .ok()
    });
    Some(cell)
}

/// Classify stored scalars and recover raw error labels through the shared table.
fn basic_cell(
    value: Option<CellValue>,
    error: Option<String>,
    formulas: (Option<String>, Option<String>),
) -> Option<CellObject> {
    let (kind, value, cached) = if let Some(label) = error {
        let value = ERRORS
            .iter()
            .find(|(_, s)| *s == label)
            .map_or(CellValue::Missing, |(n, _)| {
                CellValue::Number(f64::from(*n))
            });
        (ExcelDataType::Error, value, Some(label))
    } else {
        let value = value?;
        let kind = match &value {
            CellValue::Number(_) | CellValue::Date(_) => ExcelDataType::Number,
            CellValue::Text(_) => ExcelDataType::Text,
            CellValue::Boolean(_) => ExcelDataType::Boolean,
            CellValue::Missing if formulas.0.is_some() || formulas.1.is_some() => {
                ExcelDataType::Number
            }
            CellValue::Missing | CellValue::Null => return None,
        };
        (kind, value, None)
    };
    Some(CellObject {
        t: Some(kind),
        v: value,
        w: cached,
        f: formulas.0,
        array_formula: formulas.1,
        ..Default::default()
    })
}
