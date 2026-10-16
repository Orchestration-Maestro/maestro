//! Raw attachment decoder admission, independently recorded from artifact selection.
use super::decode;
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    input: Input,
    artifact: Outcome,
    overlay: Outcome,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    content: String,
}
#[derive(serde::Deserialize)]
#[serde(tag = "branch", rename_all = "lowercase", deny_unknown_fields)]
enum Outcome {
    Success { bytes: Vec<u8> },
    Error,
}
#[test]
fn pdf_raw_decoder_keeps_overlay_byte_domain() {
    let rows: Vec<Row> =
        serde_json::from_str(include_str!("../../../tests/fixtures/pdf_bytes.json")).unwrap();
    for row in rows {
        match row.overlay {
            Outcome::Success { bytes } => assert_eq!(decode(&row.input.content).unwrap(), bytes),
            Outcome::Error => assert_eq!(
                decode(&row.input.content).unwrap_err().kind(),
                std::io::ErrorKind::InvalidData
            ),
        }
        match row.artifact {
            Outcome::Success { bytes } => {
                assert_eq!(super::original(&row.input.content).unwrap(), bytes);
            }
            Outcome::Error => assert_eq!(
                super::original(&row.input.content).unwrap_err().kind(),
                std::io::ErrorKind::InvalidData
            ),
        }
    }
}
