//! Retained spreadsheet scalars and default cached text.
use super::{GeneralValue, Ssf, general};
/// Owned scalar independent of a cell's declared type.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum CellValue {
    /// Numeric value, including signed zero.
    Number(f64),
    /// Unmodified text.
    Text(String),
    /// Boolean value.
    Boolean(bool),
    /// Owned UTC date used by SDK callers.
    Date(chrono::DateTime<chrono::Utc>),
    /// Explicit null.
    Null,
    /// Absent scalar.
    #[default]
    Missing,
}
impl CellValue {
    /// Borrow the scalar for the shared renderer.
    pub(super) fn scalar(&self) -> GeneralValue<'_> {
        match self {
            Self::Number(n) => GeneralValue::Number(*n),
            Self::Text(s) => GeneralValue::Text(s),
            Self::Boolean(b) => GeneralValue::Boolean(*b),
            Self::Null | Self::Missing => GeneralValue::Empty,
            Self::Date(date) => GeneralValue::Number(super::datenum::datenum_date(date, false)),
        }
    }
}
/// Independent spreadsheet cell classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExcelDataType {
    /// Numeric cell.
    Number,
    /// String cell.
    Text,
    /// Boolean cell.
    Boolean,
    /// Error cell.
    Error,
    /// Native date cell.
    Date,
    /// Blank stub.
    Stub,
}
/// Retained raw scalar and optional formatted cache.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CellObject {
    /// Declared classification.
    pub t: Option<ExcelDataType>,
    /// Stored scalar.
    pub v: CellValue,
    /// Present formatted text, including empty text.
    pub w: Option<String>,
    /// Explicit authored or numeric format selector.
    pub z: Option<super::NumberFormat>,
    /// Native extended-format identifier fallback.
    pub xf: Option<Xf>,
}
/// Cell-only date-format preparation options.
#[derive(Debug, Default)]
pub struct CellFormatOptions<'a> {
    /// Replacement selector for date cells without an active explicit format.
    pub date_nf: Option<&'a super::NumberFormat>,
}
/// Extended-format metadata retained by a cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Xf {
    /// Numeric format identifier, with zero selecting the ordinary fallback.
    pub num_fmt_id: u32,
}
/// Error codes and their standard displayed labels.
pub(super) const ERRORS: &[(u32, &str)] = &[
    (0, "#NULL!"),
    (7, "#DIV/0!"),
    (15, "#VALUE!"),
    (23, "#REF!"),
    (29, "#NAME?"),
    (36, "#NUM!"),
    (42, "#N/A"),
    (43, "#GETTING_DATA"),
    (255, "#WTF?"),
];
/// Return cached text or try the cell's explicit and extended format selectors.
/// A non-null override affects this call only; successful formatting populates the cache.
/// Date preparation consumes only the cell options, not standalone table or epoch options.
#[must_use]
pub fn format_cell(
    cell: Option<&mut CellObject>,
    value: Option<&CellValue>,
    options: &CellFormatOptions<'_>,
    ssf: &Ssf,
) -> CellValue {
    let Some(cell) = cell.filter(|c| !matches!(c.t, None | Some(ExcelDataType::Stub))) else {
        return CellValue::Text(String::new());
    };
    if let Some(text) = &cell.w {
        return CellValue::Text(text.clone());
    }
    if cell.t == Some(ExcelDataType::Date)
        && !cell.z.as_ref().is_some_and(active_format)
        && let Some(code) = options.date_nf.filter(|code| active_format(code))
    {
        cell.z = Some(code.clone());
    }
    if cell.t == Some(ExcelDataType::Error) {
        return ERRORS
            .iter()
            .find(|(code, _)| cell.v == CellValue::Number(f64::from(*code)))
            .map_or_else(
                || cell.v.clone(),
                |(_, label)| CellValue::Text((*label).to_owned()),
            );
    }
    let value = value
        .filter(|v| !matches!(v, CellValue::Null | CellValue::Missing))
        .unwrap_or(&cell.v);
    match safe_format(cell, value, ssf) {
        Ok(text) => {
            cell.w = Some(text.clone());
            CellValue::Text(text)
        }
        Err(_) => CellValue::Text(raw_text(value)),
    }
}
/// Test whether a selector participates in date-format preparation.
fn active_format(format: &super::NumberFormat) -> bool {
    match format {
        super::NumberFormat::Code(code) => !code.is_empty(),
        super::NumberFormat::Id(id) => *id != 0,
    }
}
/// Attempt explicit and extended formats without caching a failure.
fn safe_format(
    cell: &CellObject,
    value: &CellValue,
    ssf: &Ssf,
) -> Result<String, super::FormatError> {
    let date = if let (Some(ExcelDataType::Date), CellValue::Date(date)) = (cell.t, value) {
        Some(CellValue::Number(super::datenum::datenum_date(date, false)))
    } else {
        None
    };
    let value = date.as_ref().unwrap_or(value);
    let options = super::FormatOptions::default();
    if let Some(code) = &cell.z
        && let Ok(text) = ssf.format(code, value, &options)
    {
        return Ok(text);
    }
    let id = cell.xf.as_ref().map_or(0, |xf| xf.num_fmt_id);
    let id = if id == 0 && date.is_some() { 14 } else { id };
    ssf.format(&super::NumberFormat::Id(id), value, &options)
}
/// Preserve readable raw scalar spelling when neither format is supported.
fn raw_text(value: &CellValue) -> String {
    match value {
        CellValue::Number(n) => super::write_num::spelling(*n),
        CellValue::Date(date) => date.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        _ => general(value.scalar()),
    }
}
