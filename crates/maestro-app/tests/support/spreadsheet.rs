//! Strict spreadsheet recordings and complete cell projections.
#![cfg(test)]
use maestro_app::browser::ssf::{self, CellObject, CellValue, ExcelDataType};
use serde::Deserialize;
use serde_json::{Map, Value};

/// Complete source observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Corpus {
    /// Direct CSV calls.
    pub csv: Vec<Csv>,
    /// Complete attachment calls, read by the native runtime suite only.
    #[cfg(not(target_arch = "wasm32"))]
    pub attachments: Vec<Attachment>,
    /// Retained workbook observations.
    pub workbooks: Vec<Workbook>,
}
/// One CSV call.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Csv {
    /// Owning test.
    pub test: String,
    /// Input label.
    pub id: String,
    /// Supplied worksheet.
    pub input: Map<String, Value>,
    /// Expected output and mutation.
    pub expected: CsvResult,
}
/// CSV result.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CsvResult {
    /// Serialized text.
    pub csv: String,
    /// Full retained cell state.
    pub cells: Map<String, Value>,
}
/// One attachment call.
#[cfg(not(target_arch = "wasm32"))]
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
#[cfg(not(target_arch = "wasm32"))]
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
/// Raw source cell fields.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cell {
    /// Optional type.
    pub t: Option<String>,
    /// Missing and null stay distinct.
    #[serde(default = "missing")]
    pub v: Scalar,
    /// Optional formatted cache.
    pub w: Option<String>,
    /// Scalar formula.
    pub f: Option<String>,
    /// Array extent.
    #[serde(rename = "F")]
    pub array_formula: Option<String>,
}
/// Source scalar preserving negative zero.
#[derive(Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum Scalar {
    /// Tagged signed numeric spelling.
    Tagged { number: String },
    /// JSON number.
    Number(f64),
    /// String value.
    Text(String),
    /// Boolean value.
    Bool(bool),
    /// Explicit null.
    Null,
    /// Absent field, not deserializable from JSON.
    #[serde(skip)]
    Missing,
}
/// Default missing raw value.
fn missing() -> Scalar {
    Scalar::Missing
}
impl Cell {
    /// Convert the recorded native domain.
    pub fn owned(self) -> CellObject {
        let t = self.t.map(|t| match t.as_str() {
            "n" => ExcelDataType::Number,
            "s" => ExcelDataType::Text,
            "b" => ExcelDataType::Boolean,
            "e" => ExcelDataType::Error,
            "z" => ExcelDataType::Stub,
            _ => panic!("unexpected cell type"),
        });
        let v = match self.v {
            Scalar::Tagged { number } => CellValue::Number(number.parse().unwrap()),
            Scalar::Number(n) => CellValue::Number(n),
            Scalar::Text(s) => CellValue::Text(s),
            Scalar::Bool(b) => CellValue::Boolean(b),
            Scalar::Null => CellValue::Null,
            Scalar::Missing => CellValue::Missing,
        };
        CellObject {
            t,
            v,
            w: self.w,
            f: self.f,
            array_formula: self.array_formula,
            ..Default::default()
        }
    }
}
/// Load the complete fixture.
pub fn corpus() -> Corpus {
    serde_json::from_str(include_str!("../fixtures/spreadsheet/corpus.json")).unwrap()
}
/// Build the recorded worksheet without changing insertion order.
pub fn worksheet(input: Map<String, Value>) -> ssf::WorkSheet {
    let mut cells = indexmap::IndexMap::new();
    let mut range = None;
    for (key, value) in input {
        if key == "!ref" {
            range = Some(ssf::decode_range(value.as_str().unwrap()).unwrap());
        } else {
            cells.insert(key, serde_json::from_value::<Cell>(value).unwrap().owned());
        }
    }
    ssf::WorkSheet {
        cells,
        range,
        merges: Vec::new(),
    }
}
/// Compare every retained cell field, including raw signed zero.
pub fn equal_cell(actual: &CellObject, expected: &CellObject) {
    assert_eq!(actual, expected);
    if let (CellValue::Number(a), CellValue::Number(b)) = (&actual.v, &expected.v) {
        assert_eq!(a.to_bits(), b.to_bits());
    }
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
