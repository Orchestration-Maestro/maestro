//! Retained spreadsheet scalars and default cached text.
use super::{GeneralValue, Ssf, general};
/// Owned scalar independent of a cell's declared type.
#[derive(Debug, Clone, PartialEq)]
pub enum CellValue {
    /// Numeric value, including signed zero.
    Number(f64),
    /// Unmodified text.
    Text(String),
    /// Boolean value.
    Boolean(bool),
    /// Explicit null.
    Null,
    /// Absent scalar.
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
    /// Blank stub.
    Stub,
}
/// Retained raw scalar and optional formatted cache.
#[derive(Debug, Clone, PartialEq)]
pub struct CellObject {
    /// Declared classification.
    pub t: Option<ExcelDataType>,
    /// Stored scalar.
    pub v: CellValue,
    /// Present formatted text, including empty text.
    pub w: Option<String>,
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
];
/// Return cached text or format a scalar using the current default format.
#[must_use]
pub fn format_cell(cell: Option<&mut CellObject>, ssf: &Ssf) -> CellValue {
    let Some(cell) = cell.filter(|c| !matches!(c.t, None | Some(ExcelDataType::Stub))) else {
        return CellValue::Text(String::new());
    };
    if let Some(text) = &cell.w {
        return CellValue::Text(text.clone());
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
    match ssf.format(0, cell.v.scalar(), false) {
        Ok(text) => {
            cell.w = Some(text.clone());
            CellValue::Text(text)
        }
        Err(_) => CellValue::Text(general(cell.v.scalar())),
    }
}
