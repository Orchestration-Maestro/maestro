//! Embedded bytes exercised through retained package relationships.
#![cfg(test)]
use maestro_app::browser::docx_preview::{DocxError, Options, PartKind, WordDocument};
use serde_json::{Value, json};
use std::io::{Cursor, Write};
/// Replace the opaque entry in the existing controlled package.
fn with_payload(data: &[u8]) -> Vec<u8> {
    let mut source = zip::ZipArchive::new(Cursor::new(include_bytes!(
        "../fixtures/docx/font-present.docx"
    )))
    .unwrap();
    let mut target = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..source.len() {
        let entry = source.by_index(index).unwrap();
        if entry.name().unwrap() != "word/fonts/opaque.bin" {
            target.raw_copy_file(entry).unwrap();
        }
    }
    target
        .start_file(
            "word/fonts/opaque.bin",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
    target.write_all(data).unwrap();
    target.finish().unwrap().into_inner()
}
/// Read one font without adding a public transformation-only API.
pub(super) fn observe(
    resource: &str,
    data: Option<&[u8]>,
    key: Option<&str>,
    reload: bool,
) -> Value {
    let bytes = match data {
        Some(data) => with_payload(data),
        None => match resource {
            "present" => include_bytes!("../fixtures/docx/font-present.docx").to_vec(),
            "empty" => include_bytes!("../fixtures/docx/font-empty.docx").to_vec(),
            "missingRelationship" => {
                include_bytes!("../fixtures/docx/font-missing-relationship.docx").to_vec()
            }
            "missingEntry" => include_bytes!("../fixtures/docx/font-missing-entry.docx").to_vec(),
            _ => panic!("Unknown resource selection"),
        },
    };
    let document = WordDocument::load(&bytes, Options::default()).unwrap();
    let part = document
        .parts()
        .iter()
        .find(|p| matches!(p.kind, PartKind::FontTable))
        .unwrap();
    match document.load_font(part, "font", key) {
        Ok(bytes) => {
            if let Some(data) = data {
                let decoded = bytes.as_ref().unwrap();
                assert_eq!(decoded.len(), data.len());
                assert_eq!(decoded.get(32..), data.get(32..));
            }
            let mut out = json!({"bytes":bytes});
            if reload {
                out["again"] = json!(document.load_font(part, "font", key).unwrap());
                out["raw"] = json!(document.load_resource(part, "font").unwrap());
            }
            out
        }
        Err(DocxError::FontKey) => {
            assert_eq!(
                DocxError::FontKey.to_string(),
                "Embedded font key is missing"
            );
            json!({"error":"TypeError"})
        }
        Err(DocxError::Archive(_) | DocxError::Xml { .. } | DocxError::Output(_)) => {
            panic!("Unexpected font resource failure")
        }
    }
}
