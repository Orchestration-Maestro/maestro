//! Native spreadsheet recordings: attachment calls, retained workbook projections and
//! repository worksheet bytes.
#![cfg(test)]
use super::spreadsheet::Cell;
use serde::Deserialize;
use serde_json::Value;

/// One attachment call.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attachment {
    /// Owning test.
    pub test: String,
    /// Input label.
    pub id: String,
    /// Supplied source.
    pub input: Input,
    /// Complete expected result.
    pub expected: Value,
    /// Ordered literal fetch URLs.
    pub fetch_calls: Vec<String>,
}
/// Attachment input fields.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    /// Source selector.
    pub source: String,
    /// Source name.
    pub name: Option<String>,
    /// Source MIME.
    pub mime: Option<String>,
    /// Name override.
    pub r#override: Option<String>,
    /// Source URL.
    pub url: Option<String>,
    /// Repository-relative workbook bytes.
    pub file: String,
}
/// Retained reader projection.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Workbook {
    /// Repository-relative workbook bytes.
    pub file: String,
    /// Ordered sheets.
    pub expected: Vec<Sheet>,
}
/// Expected sheet state.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sheet {
    /// Sheet name.
    pub name: String,
    /// Declared range.
    pub range: Option<String>,
    /// Ordered cells.
    pub cells: indexmap::IndexMap<String, Cell>,
}
/// Load the complete attachment calls.
pub fn attachments() -> Vec<Attachment> {
    serde_json::from_str(include_str!("../fixtures/spreadsheet/attachments.json")).unwrap()
}
/// Load the complete retained workbook projections.
pub fn workbooks() -> Vec<Workbook> {
    serde_json::from_str(include_str!("../fixtures/spreadsheet/workbooks.json")).unwrap()
}
/// Resolve a fixture from the repository root.
pub fn bytes(file: &str) -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(file),
    )
    .unwrap()
}
