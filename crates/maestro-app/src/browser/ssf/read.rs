//! XLSX values projected through workbook metadata and the shared formatter.
use super::super::docx_preview::{OpenXmlPackage, Options, Relationship, resolve_path};
use super::{
    CellAddress, CellObject, CellValue, ExcelDataType, Range, Ssf, WorkBook, WorkSheet,
    WorkbookError, decode_cell, decode_range, encode_cell, format_cell::ERRORS, styles::Package,
};
mod formulas;
use calamine::{DataType, Reader, Xls, Xlsx, XlsxError};
use indexmap::IndexMap;
use std::{collections::BTreeMap, io::Cursor};
/// Read supplied XLSX or compound-file XLS bytes into owned sheets with formatted cell text.
///
/// # Errors
/// Returns contextual archive, missing-part, XML, native-reader or address failures.
pub fn read(data: &[u8], workbook_name: &str, ssf: &mut Ssf) -> Result<WorkBook, WorkbookError> {
    if data.starts_with(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]) {
        return read_legacy(data, workbook_name, ssf);
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
                sheet(&package, &path, values, (&xfs, ssf, date1904), &shared)?,
            );
        }
    }
    Ok(WorkBook {
        sheets,
        date1904,
        ssf: ssf
            .table
            .read()
            .map_or_else(|error| error.into_inner().clone(), |table| table.clone()),
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
                let Some(value) = native_scalar(cell.get_value()) else {
                    continue;
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
    shared: &[String],
) -> Result<WorkSheet, WorkbookError> {
    let text = package.text(path)?;
    let xml = package.xml(&text, path)?;
    let mut result = WorkSheet {
        cells: IndexMap::new(),
        range: None,
        merges: Vec::new(),
    };
    let mut arrays = Vec::new();
    let mut shared = formulas::Shared::new();
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
            let formula = formulas::formula(&xml, node, position, &mut shared)?;
            let array = formulas::array_range(&xml, node, position, &mut arrays)?;
            let value = values.remove(&(position.r, position.c));
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
/// Overlay authored strings and missing formula caches on native scalar values.
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

/// Classify owned native scalars and recover raw error labels through the shared table.
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
/// Project both borrowed modern and owned legacy native scalars without display coercion.
fn native_scalar(value: &impl DataType) -> Option<CellValue> {
    use num_traits::ToPrimitive;
    if let Some(number) = value
        .get_float()
        .or_else(|| value.get_int()?.to_f64())
        .or_else(|| value.get_datetime().map(|date| date.as_f64()))
    {
        Some(CellValue::Number(number))
    } else if let Some(text) = value
        .get_string()
        .or_else(|| value.get_datetime_iso())
        .or_else(|| value.get_duration_iso())
    {
        Some(CellValue::Text(text.to_owned()))
    } else {
        value.get_bool().map(CellValue::Boolean)
    }
}
/// Read legacy native values while retaining the existing live format owner.
fn read_legacy(data: &[u8], name: &str, ssf: &Ssf) -> Result<WorkBook, WorkbookError> {
    let mut native = Xls::new(Cursor::new(data))
        .map_err(|error| WorkbookError::Reader(format!("{name}: {error}")))?;
    let date1904 = native.has_1904_epoch();
    let mut sheets = IndexMap::new();
    for sheet_name in native.sheet_names() {
        let values = native
            .worksheet_range(&sheet_name)
            .map_err(|error| WorkbookError::Reader(format!("{name}: {sheet_name}: {error}")))?;
        let merges = native
            .merge_cells_by_sheet_name(&sheet_name)
            .map_err(|error| WorkbookError::Reader(format!("{name}: {sheet_name}: {error}")))?;
        sheets.insert(sheet_name, legacy_sheet(&values, &merges)?);
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
/// Preserve native populated bounds, absolute positions, merge order and raw text.
fn legacy_sheet(
    values: &calamine::Range<calamine::Data>,
    merges: &[calamine::Dimensions],
) -> Result<WorkSheet, WorkbookError> {
    let mut cells = IndexMap::new();
    let start = values.start().unwrap_or_default();
    for (row, column, value) in values.used_cells() {
        let error = value.get_error().map(ToString::to_string);
        if let Some(mut cell) = basic_cell(native_scalar(value), error, (None, None)) {
            cell.w = cell.w.or_else(|| Some(super::general(cell.v.scalar())));
            let position = CellAddress {
                r: legacy_offset(start.0, row)?,
                c: legacy_offset(start.1, column)?,
            };
            cells.insert(encode_cell(position), cell);
        }
    }
    let range = values.start().zip(values.end()).map(|(s, e)| Range {
        s: CellAddress { r: s.0, c: s.1 },
        e: CellAddress { r: e.0, c: e.1 },
    });
    let merges = merges
        .iter()
        .map(|bounds| Range {
            s: CellAddress {
                r: bounds.start.0,
                c: bounds.start.1,
            },
            e: CellAddress {
                r: bounds.end.0,
                c: bounds.end.1,
            },
        })
        .collect();
    Ok(WorkSheet {
        cells,
        range,
        merges,
    })
}
/// Add a native relative cell offset without truncating its address.
fn legacy_offset(start: u32, offset: usize) -> Result<u32, WorkbookError> {
    u32::try_from(offset)
        .ok()
        .and_then(|n| start.checked_add(n))
        .ok_or_else(|| WorkbookError::Address("legacy cell address outside numeric range".into()))
}

/// Decode raw spreadsheet string escapes in one pass over UTF-16 code units.
pub(super) fn string_cell(value: &str) -> String {
    let mut rest = value;
    let mut units = Vec::new();
    while let Some(c) = rest.chars().next() {
        let escaped = rest
            .get(..7)
            .filter(|s| {
                (s.starts_with("_x") || s.starts_with("_X"))
                    && s.ends_with('_')
                    && s[2..6].bytes().all(|b| b.is_ascii_hexdigit())
            })
            .and_then(|s| u16::from_str_radix(&s[2..6], 16).ok());
        if let Some(unit) = escaped {
            units.push(unit);
            rest = &rest[7..];
        } else {
            units.extend_from_slice(c.encode_utf16(&mut [0; 2]));
            rest = &rest[c.len_utf8()..];
        }
    }
    String::from_utf16_lossy(&units)
}
