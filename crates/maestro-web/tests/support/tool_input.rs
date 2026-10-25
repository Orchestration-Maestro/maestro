//! Recorded operands shared by browser tool-card witnesses.
#![cfg(test)]
use maestro_app::JsonObject;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Input {
    pub params: Option<JsonObject>,
    pub result: Option<maestro_app::ToolResultMessage>,
    pub is_streaming: bool,
    pub language: Option<String>,
}
