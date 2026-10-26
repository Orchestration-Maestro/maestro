//! Recorded document operands and the shared native containing receiver.
#![cfg(test)]
use crate::support::tool_input::Input;
use maestro_web::tools::types::{ToolRenderResult, ToolUiError};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Row {
    pub id: String,
    pub group: String,
    pub tool: String,
    pub test: Option<String>,
    pub input: Input,
    pub expected: Option<Expected>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Expected {
    pub caption: String,
    pub status: String,
    pub spinner: usize,
    pub collapsible: bool,
    pub url: Option<String>,
    pub blocks: Vec<Block>,
    pub console: Vec<crate::document_card_support::Console>,
    pub attachments: Vec<DownloadExpectation>,
}
/// Expected downloadable fields; generated identity is intentionally not projected.
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct DownloadExpectation {
    pub r#type: maestro_app::browser::utils::attachment_utils::AttachmentType,
    pub file_name: String,
    pub mime_type: String,
    pub size: usize,
    pub content: String,
    pub preview: Option<String>,
    pub extracted_text: Option<String>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Block {
    pub code: String,
    pub language: String,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Console {
    pub content: String,
    pub variant: String,
}
pub fn fixture() -> Vec<Row> {
    serde_json::from_str(include_str!("../fixtures/document_tool_cards.json")).unwrap()
}
pub fn prepare(row: &Row) -> Result<ToolRenderResult, ToolUiError> {
    maestro_web::render_tool(
        name(&row.tool),
        row.input
            .params
            .as_ref()
            .map(maestro_web::tools::types::ToolParams::object),
        row.input.result.as_ref(),
        row.input.is_streaming,
    )
}
pub fn name(tool: &str) -> &'static str {
    match tool {
        "repl" => "javascript_repl",
        "extract" => "extract_document",
        _ => panic!("unknown document tool"),
    }
}
#[cfg(target_arch = "wasm32")]
pub mod browser;
#[cfg(target_arch = "wasm32")]
pub mod forwarding;

/// Select distinct file-list inputs while retaining each recorded file's independent oracle.
#[cfg(target_arch = "wasm32")]
pub fn file_subset(indices: &[usize]) -> Row {
    let mut row = fixture()
        .into_iter()
        .find(|r| r.id == "repl-files")
        .unwrap();
    let result = row.input.result.as_mut().unwrap();
    let files = result.details.as_ref().unwrap()["files"]
        .as_array()
        .unwrap();
    let selected = indices.iter().map(|i| files[*i].clone()).collect();
    result.details.as_mut().unwrap()["files"] = serde_json::Value::Array(selected);
    let expected = &mut row.expected.as_mut().unwrap().attachments;
    *expected = indices.iter().map(|i| expected[*i].clone()).collect();
    row
}
#[cfg(target_arch = "wasm32")]
pub mod lifecycle;

#[cfg(target_arch = "wasm32")]
pub mod overlays;
#[cfg(target_arch = "wasm32")]
pub mod styles;

#[cfg(target_arch = "wasm32")]
pub mod downloads;
