//! Shared workbook fixture byte loading.
pub fn bytes(file: &str) -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/workbooks")
            .join(file),
    )
    .unwrap()
}
