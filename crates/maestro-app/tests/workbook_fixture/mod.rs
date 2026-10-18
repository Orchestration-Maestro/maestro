//! Shared workbook fixture bytes and recorded cell ranges.
use maestro_app::browser::ssf;
use serde::Deserialize;

pub fn bytes(file: &str) -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/workbooks")
            .join(file),
    )
    .unwrap()
}
/// One recorded cell position.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Position {
    r: u32,
    c: u32,
}
/// One recorded merge, as start and end positions.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bounds {
    s: Position,
    e: Position,
}
impl Bounds {
    /// The same extent as a reader range.
    pub fn range(&self) -> ssf::Range {
        ssf::Range {
            s: ssf::CellAddress {
                r: self.s.r,
                c: self.s.c,
            },
            e: ssf::CellAddress {
                r: self.e.r,
                c: self.e.c,
            },
        }
    }
}
