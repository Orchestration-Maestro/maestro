//! Recorded whole-root protocol inputs and observations.
#![cfg(not(target_arch = "wasm32"))]
use crate::chat::TestResult;
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Routes {
    pub reasoning: Vec<Reasoning>,
    pub history: Vec<History>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reasoning {
    pub chunks: Vec<String>,
    pub expected: ReasoningOutput,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReasoningOutput {
    pub events: Vec<Value>,
    pub content: Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct History {
    #[serde(default)]
    pub input: Value,
    pub input_json: Option<String>,
    pub expected: HistoryOutput,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoryOutput {
    pub chat: Value,
    pub responses: Value,
    pub message: Value,
    pub content: Value,
    pub cloud: Value,
    pub reasoning: Value,
}

pub fn recorded() -> TestResult<Routes> {
    Ok(serde_json::from_str(include_str!(
        "../fixtures/tool_argument_root_routes.json"
    ))?)
}
