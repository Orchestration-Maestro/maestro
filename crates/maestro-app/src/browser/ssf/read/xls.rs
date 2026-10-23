//! Legacy binary workbook reading into the shared workbook records.
mod formulas;
mod hyperlink;
mod records;
mod strings;
use super::{
    super::{
        CellAddress, CellObject, CellValue, ExcelDataType, Range, Ssf, WorkBook, WorkSheet,
        WorkbookError, encode_cell, format_cell::error_label, styles::repair_format,
    },
    blob::{ParseError, Result, index},
    cfb,
};
use indexmap::IndexMap;
use records::{Bounds, CellHeader, EOF, FormulaValue, Payload, Record, Records};
use std::collections::{BTreeMap, HashMap};
use strings::Encoding;

/// Beginning-of-substream record.
const BOF: u16 = 0x0809;
/// Substream type of a chart sheet.
const CHART: u16 = 0x0020;

/// Scalar decoded from a cell record.
enum Value {
    /// Number.
    Number(f64),
    /// Text.
    Text(String),
    /// Boolean.
    Boolean(bool),
    /// Error code.
    Error(u8),
}

/// Substream being read.
#[derive(Default)]
struct Sheet {
    /// Offset of its beginning record, matched against sheet directory positions.
    offset: usize,
    /// Chart sheets shift a repeated number one column right.
    chart: bool,
    /// Cells in record order.
    cells: IndexMap<String, CellObject>,
    /// Declared or observed bounds with exclusive ends.
    bounds: Option<Bounds>,
    /// Per-sheet retained templates and current formula identity.
    formulas: formulas::FormulaState,
    /// Merged rectangles in record order.
    merges: Vec<Range>,
}

/// One linear pass over the workbook stream.
struct Reader<'s> {
    /// Live format table receiving number formats.
    ssf: &'s Ssf,
    /// String representation, fixed by the first beginning record.
    encoding: Option<Encoding>,
    /// Workbook date epoch.
    date1904: bool,
    /// Number-format identifier of each extended format.
    xfs: Vec<u16>,
    /// Shared-string table.
    strings: Vec<String>,
    /// Sheet names by substream position.
    directory: BTreeMap<u32, usize>,
    /// Substream nesting depth.
    depth: u32,
    /// Current top-level substream.
    sheet: Sheet,
    /// Finished substreams by offset.
    finished: HashMap<usize, Sheet>,
    /// Canonical workbook name context.
    books: formulas::SupBooks,
}

/// Read a compound-file workbook into owned sheets, loading its formats into `ssf`.
pub(super) fn read(
    data: &[u8],
    name: &str,
    ssf: &Ssf,
) -> std::result::Result<WorkBook, WorkbookError> {
    workbook(data, ssf).map_err(|error| WorkbookError::Reader(format!("{name}: {error}")))
}

/// Parse every record once, then order sheets by their directory positions.
fn workbook(data: &[u8], ssf: &Ssf) -> Result<WorkBook> {
    let stream = cfb::workbook_stream(data)?;
    let mut reader = Reader {
        ssf,
        encoding: None,
        date1904: false,
        xfs: Vec::new(),
        strings: Vec::new(),
        directory: BTreeMap::new(),
        depth: 0,
        sheet: Sheet::default(),
        finished: HashMap::new(),
        books: formulas::SupBooks::default(),
    };
    let mut records = Records::new(&stream);
    while let Some(record) = records.next()? {
        if reader.depth == 0 && record.kind != BOF {
            break;
        }
        reader.record(&record)?;
    }
    let mut sheets = IndexMap::new();
    for (position, name) in std::mem::take(&mut reader.directory) {
        let name = std::mem::take(&mut reader.books.sheet_names[name]);
        let sheet = reader
            .finished
            .remove(&index(position)?)
            .ok_or(ParseError::Invalid("sheet position"))?;
        sheets.insert(name, Ok(sheet.finish()));
    }
    let table = crate::utils::lock::read(&ssf.table).clone();
    Ok(WorkBook {
        sheets,
        date1904: reader.date1904,
        ssf: table,
    })
}

impl Sheet {
    /// Project bounds the way the workbook model stores them.
    fn finish(self) -> WorkSheet {
        let range = self
            .bounds
            .filter(|(_, end)| end.0 > 0 && end.1 > 0)
            .map(|(start, end)| Range {
                s: CellAddress {
                    r: start.0,
                    c: start.1,
                },
                e: CellAddress {
                    r: end.0 - 1,
                    c: end.1 - 1,
                },
            });
        WorkSheet {
            cells: self.cells,
            range,
            merges: self.merges,
        }
    }
}

impl Reader<'_> {
    /// String representation, defaulting to the newer layout before any beginning record.
    fn encoding(&self) -> Encoding {
        self.encoding.unwrap_or(Encoding::Unicode)
    }
    /// Apply one record.
    fn record(&mut self, record: &Record<'_>) -> Result<()> {
        let mut payload = record.payload();
        match record.kind {
            BOF => self.begin(&mut payload, record.offset)?,
            EOF => self.end(),
            0x0022 => self.date1904 = payload.u16()? == 1,
            0x002f => return Err(ParseError::Encrypted),
            0x0042 => self.code_page(payload.u16()?),
            0x00e0 => {
                payload.u16()?;
                self.xfs.push(payload.u16()?);
            }
            0x01b8 => hyperlink::apply(&mut payload, &mut self.sheet.cells)?,
            0x0021 | 0x0221 => {
                self.sheet.formulas.array(&mut payload, self.encoding())?;
                self.expression()?;
            }
            0x04bc => {
                self.sheet.formulas.shared(&mut payload, self.encoding())?;
                self.expression()?;
            }
            0x0023 | 0x0223 => self.books.external_name(&mut payload, self.encoding())?,
            0x01ae => self.books.book(&mut payload)?,
            0x0017 => self.books.external_sheets(&mut payload, self.encoding())?,
            0x0018 | 0x0218 => self.books.label(&mut payload, self.encoding())?,
            0x0085 => self.bound_sheet(&mut payload)?,
            0x041e => {
                let id = payload.u16()?;
                let code = strings::format_string(&mut payload, self.encoding())?;
                let _ = self.ssf.load(repair_format(&code), Some(u32::from(id)));
            }
            0x00fc => self.strings = strings::shared_strings(record)?,
            0x0200 if self.depth == 1 => {
                let wide = self.encoding() == Encoding::Unicode;
                self.sheet.bounds = Some(records::dimensions(&mut payload, wide)?);
            }
            0x00e5 => {
                let merges = records::merge_cells(&mut payload)?;
                self.sheet
                    .merges
                    .extend(merges.into_iter().map(|(s, e)| Range {
                        s: CellAddress { r: s.0, c: s.1 },
                        e: CellAddress { r: e.0, c: e.1 },
                    }));
            }
            _ => self.cell_record(record.kind, &mut payload)?,
        }
        Ok(())
    }
    /// Enter a substream; the first one fixes the workbook's string representation.
    fn begin(&mut self, payload: &mut Payload<'_, '_>, offset: usize) -> Result<()> {
        let (version, kind) = records::bof(payload)?;
        if self.encoding.is_none() {
            self.encoding = Some(match (version, kind) {
                (0x0500, _) => Encoding::CodePage(1252),
                (0, 0x1000) => Encoding::CodePage(28591),
                (0x0200 | 0x0300 | 0x0400 | 0x0002 | 0x0007, _) => {
                    return Err(ParseError::Invalid("workbook version"));
                }
                _ => Encoding::Unicode,
            });
        }
        self.depth += 1;
        if self.depth == 1 {
            self.sheet = Sheet {
                offset,
                chart: kind == CHART,
                ..Sheet::default()
            };
        }
        Ok(())
    }
    /// Leave a substream, keeping a finished top-level one.
    fn end(&mut self) {
        self.depth = self.depth.saturating_sub(1);
        if self.depth == 0 {
            let sheet = std::mem::take(&mut self.sheet);
            self.finished.insert(sheet.offset, sheet);
        }
    }
    /// Apply a declared code page to byte strings, mapping producer aliases.
    fn code_page(&mut self, page: u16) {
        let page = match page {
            0x5212 => 1200,
            0x8000 => 10000,
            0x8001 => 1252,
            page => page,
        };
        if let Some(Encoding::CodePage(current)) = &mut self.encoding {
            *current = page;
        }
    }
    /// Record a sheet's position and name.
    fn bound_sheet(&mut self, payload: &mut Payload<'_, '_>) -> Result<()> {
        let position = payload.u32()?;
        payload.u16()?;
        let name = strings::short_string(payload, self.encoding())?;
        let name = if name.is_empty() {
            "Sheet1".to_owned()
        } else {
            name
        };
        self.directory
            .insert(position, self.books.sheet_names.len());
        self.books.sheet_names.push(name);
        Ok(())
    }
    /// Decode cell records into values.
    fn cell_record(&mut self, kind: u16, payload: &mut Payload<'_, '_>) -> Result<()> {
        let encoding = self.encoding();
        match kind {
            0x0203 => {
                let mut header = CellHeader::parse(payload)?;
                if self.sheet.chart && self.sheet.cells.contains_key(&address(header)) {
                    header.c += 1;
                }
                self.add(header, Value::Number(payload.f64()?), true);
            }
            0x027e => {
                let header = CellHeader::parse(payload)?;
                self.add(header, Value::Number(records::rk_number(payload)?), false);
            }
            0x00bd => {
                for (header, value) in records::mul_rk(payload)? {
                    self.add(header, Value::Number(value), false);
                }
            }
            0x0205 => {
                let header = CellHeader::parse(payload)?;
                let (value, error) = (payload.u8()?, payload.u8()? == 1);
                let value = if error {
                    Value::Error(value)
                } else {
                    Value::Boolean(value == 1)
                };
                self.add(header, value, false);
            }
            0x0006 => self.formula(payload)?,
            0x0207 => self.string(payload)?,
            0x00fd => {
                let header = CellHeader::parse(payload)?;
                let text = self.strings.get(index(payload.u32()?)?);
                let text = text
                    .ok_or(ParseError::Invalid("shared string index"))?
                    .clone();
                self.add(header, Value::Text(text), false);
            }
            0x0204 => {
                let header = CellHeader::parse(payload)?;
                let text = strings::unicode_string(payload, encoding)?;
                self.add(header, Value::Text(text), false);
            }
            0x00d6 => {
                let header = CellHeader::parse(payload)?;
                let count = usize::from(payload.u16()?);
                self.add(header, Value::Text(encoding.text(payload, count)?), false);
            }
            _ => {}
        }
        Ok(())
    }
    /// Materialize a pending formula's string cache without losing its template.
    fn string(&mut self, payload: &mut Payload<'_, '_>) -> Result<()> {
        let last = self
            .sheet
            .formulas
            .last
            .as_ref()
            .filter(|last| last.pending)
            .ok_or(ParseError::Invalid("string record"))?;
        let header = last.header;
        let text = strings::unicode_string(payload, self.encoding())?;
        self.add(header, Value::Text(text), false);
        self.expression()?;
        self.sheet.formulas.last = None;
        Ok(())
    }
    /// Add a cached formula result, or wait for the string record that carries it.
    fn formula(&mut self, payload: &mut Payload<'_, '_>) -> Result<()> {
        let header = CellHeader::parse(payload)?;
        let cached = FormulaValue::parse(payload)?;
        payload.skip(6)?;
        let tokens = formulas::parse(payload, self.encoding())?;
        let pending = matches!(cached, FormulaValue::PendingString);
        self.sheet.formulas.formula(header, tokens, pending);
        if pending {
            return Ok(());
        }
        let value = match cached {
            FormulaValue::Number(n) => Value::Number(n),
            FormulaValue::Boolean(b) => Value::Boolean(b),
            FormulaValue::Error(code) => Value::Error(code),
            FormulaValue::Empty => Value::Text(String::new()),
            FormulaValue::PendingString | FormulaValue::Unknown => return Ok(()),
        };
        self.add(header, value, false);
        self.expression()
    }
    /// Attach a decoded expression to its materialized cache.
    fn expression(&mut self) -> Result<()> {
        let Some(last) = &self.sheet.formulas.last else {
            return Ok(());
        };
        let encoding = self.encoding();
        if let Some(cell) = self.sheet.cells.get_mut(&address(last.header)) {
            self.sheet.formulas.apply(cell, &self.books, encoding)?;
        }
        Ok(())
    }
    /// Format and place one top-level cell, growing the sheet bounds.
    fn add(&mut self, header: CellHeader, value: Value, number: bool) {
        if self.depth > 1 {
            return;
        }
        let id = format_id(&self.xfs, header.ixfe, number);
        let mut cell = cell(value);
        safe_format_xf(&mut cell, id, self.ssf, self.date1904);
        let bounds = self.sheet.bounds.get_or_insert(((0, 0), (0, 0)));
        bounds.0 = (bounds.0.0.min(header.r), bounds.0.1.min(header.c));
        bounds.1 = (bounds.1.0.max(header.r + 1), bounds.1.1.max(header.c + 1));
        self.sheet.cells.insert(address(header), cell);
    }
}

/// Encoded address of a cell header.
fn address(header: CellHeader) -> String {
    encode_cell(CellAddress {
        r: header.r,
        c: header.c,
    })
}

/// Classify a decoded scalar, giving errors their label.
fn cell(value: Value) -> CellObject {
    let (t, v, w) = match value {
        Value::Number(n) => (ExcelDataType::Number, CellValue::Number(n), None),
        Value::Text(s) => (ExcelDataType::Text, CellValue::Text(s), None),
        Value::Boolean(b) => (ExcelDataType::Boolean, CellValue::Boolean(b), None),
        Value::Error(code) => (
            ExcelDataType::Error,
            CellValue::Number(f64::from(code)),
            error_label(&CellValue::Number(f64::from(code))).map(str::to_owned),
        ),
    };
    CellObject {
        t: Some(t),
        v,
        w,
        ..CellObject::default()
    }
}

/// Leave unresolved formats uncached and keep General independent of slot zero.
fn safe_format_xf(cell: &mut CellObject, id: Option<u16>, ssf: &Ssf, date1904: bool) {
    let Some(id) = id else {
        cell.w = None;
        return;
    };
    if cell.t == Some(ExcelDataType::Error) {
        return;
    }
    cell.w = if id == 0 {
        Some(super::super::general(cell.v.scalar()))
    } else {
        ssf.format(
            &super::super::NumberFormat::Id(u32::from(id)),
            &cell.v,
            &super::super::FormatOptions {
                date1904,
                ..Default::default()
            },
        )
        .ok()
    };
}

/// Resolve a cell's extended format, defaulting only NUMBER records to General.
fn format_id(xfs: &[u16], ixfe: u16, number: bool) -> Option<u16> {
    xfs.get(usize::from(ixfe)).copied().or(number.then_some(0))
}

#[cfg(test)]
mod tests;
