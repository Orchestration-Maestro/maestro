//! Recorded artifact rendering operands and selected native preparation.
#![cfg(test)]
use maestro_web::{
    ArtifactsToolRenderer, ToolRenderer,
    tools::types::{ToolRenderResult, ToolUiError},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Row {
    pub input: crate::support::tool_input::Input,
    pub expected: Expected,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Expected {
    pub is_custom: bool,
    pub status: String,
    pub caption: String,
    pub pill: Option<String>,
    pub collapsible: bool,
    pub initially_collapsed: Option<bool>,
    pub code: Vec<Code>,
    pub console: Vec<Console>,
    pub errors: Vec<String>,
    pub diff: Vec<DiffRow>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Code {
    pub code: String,
    pub language: String,
    pub decoded: String,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Console {
    pub content: String,
    pub variant: String,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiffRow {
    pub text: String,
    pub kind: String,
}
/// Read one recorded command group.
/// # Panics
/// Panics if a committed fixture is malformed or the group is absent.
#[must_use]
pub fn rows(name: &str) -> Vec<Row> {
    let mut groups: BTreeMap<String, Vec<Row>> =
        serde_json::from_str(include_str!("../fixtures/artifact_tool_cards.json")).unwrap();
    groups.remove(name).unwrap()
}
/// Prepare a card through the borrowed object adapter.
/// # Errors
/// Returns the selected consumer error.
pub fn prepare(row: &Row) -> Result<ToolRenderResult, ToolUiError> {
    ArtifactsToolRenderer::new(None).render(
        row.input.params.as_ref(),
        row.input.result.as_ref().map(Into::into),
        row.input.is_streaming,
    )
}
/// Construct the recorded result phase.
/// # Panics
/// Panics if the supplied phase is unknown.
#[must_use]
pub fn phase(phase: &str) -> Option<maestro_app::ToolResultMessage> {
    match phase {
        "pending" => None,
        "success" | "error" => {
            let mut result = crate::recipes::message(None);
            result.content.clear();
            result.is_error = phase == "error";
            Some(result)
        }
        _ => panic!("unknown phase"),
    }
}
/// Prepare a card through the whole-value adapter.
/// # Errors
/// Returns the selected consumer error.
/// # Panics
/// Panics if the supplied phase is unknown.
pub fn value(params: Option<&Value>, phase_name: &str) -> Result<ToolRenderResult, ToolUiError> {
    let result = phase(phase_name);
    ArtifactsToolRenderer::new(None).render(params, result.as_ref().map(Into::into), false)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Language {
    pub input: Filename,
    pub expected: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Filename {
    pub filename: Option<String>,
}
/// Read the literal suffix corpus.
/// # Panics
/// Panics if a committed fixture is malformed or the group is absent.
#[must_use]
pub fn languages() -> Vec<Language> {
    serde_json::from_str(include_str!("../fixtures/artifact_languages.json")).unwrap()
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Admission {
    pub key: String,
    pub kind: String,
    pub phase: String,
    pub context: String,
    pub input: Value,
    pub expected: Branch,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, tag = "branch")]
pub enum Branch {
    #[serde(rename = "prepared")]
    Prepared {
        #[serde(rename = "isCustom")]
        is_custom: bool,
    },
    #[serde(rename = "rendered")]
    Rendered { card: Expected },
    #[serde(rename = "error")]
    Error { kind: String },
}
/// Read reached-field consumer outcomes.
/// # Panics
/// Panics if a committed fixture is malformed or the group is absent.
#[must_use]
pub fn admission() -> Vec<Admission> {
    serde_json::from_str(include_str!("../fixtures/artifact_field_admission.json")).unwrap()
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValueRoute {
    pub input: PhaseInput,
    pub expected: Expected,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhaseInput {
    pub params: Option<Value>,
    pub phase: String,
}
/// Read whole-value parameter routes.
/// # Panics
/// Panics if a committed fixture is malformed or the group is absent.
#[must_use]
pub fn value_routes() -> Vec<ValueRoute> {
    serde_json::from_str(include_str!("../fixtures/artifact_value_routes.json")).unwrap()
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnusedDiff {
    pub input: crate::support::tool_input::Input,
    pub expected: Branch,
}
/// Read discarded and reached replacement controls.
/// # Panics
/// Panics if a committed fixture is malformed or the group is absent.
#[must_use]
pub fn unused_diffs() -> Vec<UnusedDiff> {
    serde_json::from_str(include_str!("../fixtures/artifact_unused_diff.json")).unwrap()
}
#[cfg(target_arch = "wasm32")]
pub mod browser;
#[cfg(target_arch = "wasm32")]
pub mod ownership;
#[cfg(target_arch = "wasm32")]
pub mod pills;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodeDomain {
    pub input: CodeInput,
    pub expected: CodeSelection,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodeInput {
    pub kind: String,
    pub phase: String,
    pub params: Value,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, tag = "branch")]
pub enum CodeSelection {
    #[serde(rename = "omitted")]
    Omitted,
    #[serde(rename = "text")]
    Text { text: String },
}
/// Read the selected-code value corpus.
/// # Panics
/// Panics if a committed fixture is malformed or the group is absent.
#[must_use]
pub fn code_domains() -> Vec<CodeDomain> {
    serde_json::from_str(include_str!("../fixtures/artifact_code_domains.json")).unwrap()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiffCase {
    pub input: DiffInput,
    pub expected: DiffExpected,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct DiffInput {
    pub old_text: String,
    pub new_text: String,
    pub title: Option<String>,
    pub class_name: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct DiffExpected {
    pub rows: Vec<DiffRow>,
    pub title: Option<String>,
    pub counts: Option<String>,
    pub class_present: bool,
}
/// Read direct diff title and line cases.
/// # Panics
/// Panics if a committed fixture is malformed or the group is absent.
#[must_use]
pub fn diff_cases() -> Vec<DiffCase> {
    serde_json::from_str(include_str!("../fixtures/artifact_diff_views.json")).unwrap()
}
#[cfg(target_arch = "wasm32")]
pub mod diff;
#[cfg(target_arch = "wasm32")]
pub mod lifecycle;

#[cfg(target_arch = "wasm32")]
pub mod composed;

#[cfg(target_arch = "wasm32")]
pub mod styles;
