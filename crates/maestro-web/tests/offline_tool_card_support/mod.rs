//! Complete offline card corpus replay through the public formatter.
#![cfg(test)]
use base64::Engine as _;
use maestro_web::export_html::template::{SessionDocument, load_document, render_tool_call};
use serde::Deserialize;
use serde_json::Value;

/// Complete recorded invocation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Row {
    pub id: String,
    pub input: Input,
    pub test: String,
    pub expected: Expected,
}
/// Raw document and selected call.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    pub call: Value,
    pub data: Value,
}
/// Success observations or reached invalid data.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Expected {
    pub branch: String,
    pub html: Option<String>,
    pub text: Option<String>,
    pub status: Option<String>,
    pub attributes: Option<Vec<Vec<(String, String)>>>,
    pub line_ranges: Option<Vec<String>>,
    pub expandable: Option<usize>,
    pub error_kind: Option<String>,
}
/// Load an empty document, then retain raw reached operands for card-local validation.
pub fn document(data: &Value) -> SessionDocument {
    let bytes = base64::engine::general_purpose::STANDARD.encode(b"{\"entries\":[]}");
    let mut doc = load_document(&bytes, "", None).unwrap();
    doc.data = data.as_object().unwrap().clone();
    doc
}
/// Replay exactly one witness's complete selected corpus.
pub fn replay(test: &str) {
    let rows: Vec<Row> =
        serde_json::from_str(include_str!("../fixtures/offline_tool_cards.json")).unwrap();
    let rows: Vec<_> = rows.into_iter().filter(|row| row.test == test).collect();
    assert!(!rows.is_empty());
    let engine = maestro_theme::SyntectHighlighter::new().unwrap();
    for row in rows {
        let doc = document(&row.input.data);
        let actual = render_tool_call(&doc, &row.input.call, &engine);
        if row.expected.branch == "error" {
            assert_eq!(row.expected.error_kind.as_deref(), Some("InvalidData"));
            assert_eq!(
                actual.unwrap_err().kind(),
                std::io::ErrorKind::InvalidData,
                "{}",
                row.id
            );
            let call =
                serde_json::json!({"id":"neighbor","name":"bash","arguments":{"command":"intact"}});
            assert_eq!(
                render_tool_call(&doc, &call, &engine).unwrap(),
                "<div class=\"tool-execution pending\"><div class=\"tool-command\">$ intact</div></div>"
            );
        } else {
            let actual = actual.unwrap();
            compare(&actual, &row.expected, &row.id);
        }
    }
}
/// Compare complete markup without discarding custom spans or their visible whitespace styles.
#[cfg(not(target_arch = "wasm32"))]
fn compare(actual: &str, expected: &Expected, id: &str) {
    crate::export_support::assert_markup(actual, expected.html.as_deref().unwrap(), id);
    // Native markup is the complete observation; these decoded fields are checked in the browser.
    let _ = (
        &expected.text,
        &expected.status,
        &expected.attributes,
        &expected.line_ranges,
        &expected.expandable,
    );
}
/// Observe ordered native elements and every retained decoded field.
#[cfg(target_arch = "wasm32")]
fn compare(actual: &str, expected: &Expected, id: &str) {
    crate::export_support::assert_markup(actual, expected.html.as_deref().unwrap(), id);
    let root = web_sys::window()
        .unwrap()
        .document()
        .unwrap()
        .create_element("div")
        .unwrap();
    root.set_inner_html(actual);
    let observation = crate::reading_slots_support::js(
        &root,
        "return {text:value.textContent,status:value.firstElementChild.className,attributes:[...value.querySelectorAll('img')].map(e=>[...e.attributes].map(a=>[a.name,a.value])),lineRanges:[...value.querySelectorAll('.line-numbers')].map(e=>e.textContent),expandable:value.querySelectorAll('.expandable').length}",
    );
    let actual: Value = serde_json::from_str(
        &js_sys::JSON::stringify(&observation)
            .unwrap()
            .as_string()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        actual,
        serde_json::json!({"text":crate::export_support::normalize_indent(expected.text.as_deref().unwrap()),"status":expected.status,"attributes":expected.attributes,"lineRanges":expected.line_ranges,"expandable":expected.expandable}),
        "{id}"
    );
}
