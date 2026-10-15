//! Owned workbook records and reader failures.
use super::{CellObject, Range};
use indexmap::IndexMap;
use std::collections::BTreeMap;
/// Worksheet cells in input order with declared or observed extent.
#[derive(Debug)]
pub struct WorkSheet {
    /// Populated cells keyed by their rendered address.
    pub cells: IndexMap<String, CellObject>,
    /// Used range when present.
    pub range: Option<Range>,
    /// Merged regions in input order.
    pub merges: Vec<Range>,
}
/// Owned workbook projection.
#[derive(Debug)]
pub struct WorkBook {
    /// Sheets in workbook order.
    pub sheets: IndexMap<String, WorkSheet>,
    /// Whether serials use the 1904 epoch.
    pub date1904: bool,
    /// Detached number-format table snapshot.
    pub ssf: BTreeMap<u32, String>,
}
/// Failure at an archive, XML, native reader or address boundary.
#[derive(Debug)]
pub enum WorkbookError {
    /// Invalid archive with supplied workbook context.
    Archive(String),
    /// Required part absent from the archive.
    MissingPart(String),
    /// Malformed XML with part context.
    Xml(String),
    /// Native value-reader failure.
    Reader(String),
    /// Invalid cell or range address.
    Address(String),
}
impl std::fmt::Display for WorkbookError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Archive(s)
            | Self::MissingPart(s)
            | Self::Xml(s)
            | Self::Reader(s)
            | Self::Address(s) => f.write_str(s),
        }
    }
}
impl std::error::Error for WorkbookError {}
