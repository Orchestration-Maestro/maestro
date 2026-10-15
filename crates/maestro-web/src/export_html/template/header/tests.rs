//! Recorded literal identity operands consumed by native mounting.
use super::*;
use base64::Engine as _;
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    name: String,
    data: serde_json::Value,
    title: String,
}
fn document(value: &serde_json::Value) -> SessionDocument {
    super::super::load_document(
        &base64::engine::general_purpose::STANDARD.encode(value.to_string()),
        "",
        None,
    )
    .unwrap()
}
#[test]
fn header_identity_stays_literal_and_uses_truthy_fallback() {
    let rows: Vec<Row> = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/session_identity.json"
    ))
    .unwrap();
    assert_eq!(rows.len(), 6);
    for row in rows {
        assert_eq!(
            prepare(&document(&row.data)).unwrap(),
            row.title,
            "{}",
            row.name
        );
    }
    let text = "  <script>雪 & \"quotes\"</script>  ";
    assert_eq!(
        prepare(&document(&json!({"entries":[],"header":{"id":text}}))).unwrap(),
        format!("Session: {text}")
    );
    for id in [json!(true), json!(12), json!({}), json!([])] {
        assert_eq!(
            prepare(&document(&json!({"entries":[],"header":{"id":id}})))
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );
    }
}
