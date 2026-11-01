//! Recorded calculation, current-time and command card operands with their observations.
#![cfg(test)]
use serde::{Deserialize, Serialize};

pub use crate::support::tool_input::Input;
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Expected {
    pub is_custom: bool,
    pub header: String,
    pub spinner: usize,
    pub status: String,
    pub console: Vec<Console>,
    pub errors: Vec<String>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Console {
    pub content: String,
    pub variant: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Row {
    pub id: String,
    pub tool: String,
    pub test: String,
    pub input: Input,
    pub expected: Expected,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Domain {
    pub tool: String,
    pub kind: String,
    pub phase: String,
    pub input: Input,
    pub expected: Expected,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Localized {
    pub tool: String,
    pub input: Input,
    pub expected: Expected,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Fixture {
    pub rows: Vec<Row>,
    pub domains: Vec<Domain>,
    pub localized: Vec<Localized>,
    pub registration: Registration,
    pub value_routes: Vec<ValueRoute>,
    pub native_detail: NativeDetail,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Registration {
    pub initial_names: Vec<String>,
    pub initial: Expected,
    pub replaced: bool,
    pub forced: Expected,
    pub restored: bool,
    pub removed: Expected,
    pub still_absent: bool,
    pub retained_original: Expected,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValueRoute {
    pub input: ValueInput,
    pub expected: Expected,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ValueInput {
    pub params: serde_json::Value,
    pub result: maestro_app::ToolResultMessage,
    pub is_streaming: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct NativeDetail {
    pub input: Input,
    pub expected: Expected,
    pub detail_reads: usize,
}

#[must_use]
pub(crate) fn fixture() -> Fixture {
    serde_json::from_str(include_str!("../fixtures/tool_cards.json")).unwrap()
}
