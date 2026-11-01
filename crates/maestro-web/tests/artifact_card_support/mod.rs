//! Recorded artifact rendering operands and selected native preparation.
#![cfg(test)]
use maestro_web::{
    ArtifactsToolRenderer, ToolRenderer,
    tools::types::{ToolRenderResult, ToolUiError},
};
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;

/// Recorded `Row` observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Row {
    /// Recorded input.
    pub input: crate::support::tool_input::Input,
    /// Recorded expected.
    pub expected: Expected,
}
/// Recorded `Expected` observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Expected {
    /// Recorded is custom.
    pub is_custom: bool,
}
/// Read one recorded command group.
/// # Panics
/// Panics if a committed fixture is malformed or the group is absent.
#[must_use]
pub fn rows(name: &str) -> Vec<Row> {
    let valid: Value =
        serde_json::from_str(include_str!("../fixtures/artifact_tool_cards.json")).unwrap();
    crate::support::strict_record::<Row>(&valid[name][0], &["input", "expected"]);
    let mut groups: BTreeMap<String, Vec<Row>> =
        serde_json::from_str(include_str!("../fixtures/artifact_tool_cards.json")).unwrap();
    groups.remove(name).unwrap()
}
/// Prepare a card through the borrowed object adapter.
/// # Errors
/// Returns the selected consumer error.
pub fn prepare(input: &crate::support::tool_input::Input) -> Result<ToolRenderResult, ToolUiError> {
    ArtifactsToolRenderer::new(None).render(
        input.params.as_ref(),
        input.result.as_ref().map(Into::into),
        input.is_streaming,
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
/// Recorded `Language` observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Language {
    /// Recorded input.
    pub input: Filename,
}
/// Recorded `Filename` observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Filename {
    /// Recorded filename.
    pub filename: Option<String>,
}
/// Read the literal suffix corpus.
/// # Panics
/// Panics if a committed fixture is malformed or the group is absent.
#[must_use]
pub fn languages() -> Vec<Language> {
    serde_json::from_str(include_str!("../fixtures/artifact_languages.json")).unwrap()
}
/// Recorded `Admission` observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Admission {
    /// Recorded key.
    pub key: String,
    /// Recorded kind.
    pub kind: String,
    /// Recorded phase.
    pub phase: String,
    /// Recorded context.
    pub context: String,
    /// Recorded input.
    pub input: Value,
    /// Recorded expected.
    pub expected: Branch,
}
/// Recorded `Branch` observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, tag = "branch")]
pub enum Branch {
    /// Recorded prepared branch.
    #[serde(rename = "prepared")]
    Prepared {
        /// Whether the selected preparation is custom.
        #[serde(rename = "isCustom")]
        is_custom: bool,
    },
    /// Recorded rendered branch.
    #[serde(rename = "rendered")]
    Rendered { card: Expected },
    /// Recorded error branch.
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
/// Recorded `ValueRoute` observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValueRoute {
    /// Recorded input.
    pub input: PhaseInput,
    /// Recorded expected.
    pub expected: Expected,
}
/// Recorded `PhaseInput` observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhaseInput {
    /// Recorded params.
    pub params: Option<Value>,
    /// Recorded phase.
    pub phase: String,
}
/// Read whole-value parameter routes.
/// # Panics
/// Panics if a committed fixture is malformed or the group is absent.
#[must_use]
pub fn value_routes() -> Vec<ValueRoute> {
    use crate::support::strict_record;
    let valid: Value =
        serde_json::from_str(include_str!("../fixtures/artifact_value_routes.json")).unwrap();
    strict_record::<ValueRoute>(&valid[0], &["input", "expected"]);
    strict_record::<PhaseInput>(&valid[0]["input"], &["phase"]);
    serde_json::from_str(include_str!("../fixtures/artifact_value_routes.json")).unwrap()
}
/// Recorded `UnusedDiff` observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnusedDiff {
    /// Recorded input.
    pub input: crate::support::tool_input::Input,
    /// Recorded expected.
    pub expected: Branch,
}
/// Read discarded and reached replacement controls.
/// # Panics
/// Panics if a committed fixture is malformed or the group is absent.
#[must_use]
pub fn unused_diffs() -> Vec<UnusedDiff> {
    serde_json::from_str(include_str!("../fixtures/artifact_unused_diff.json")).unwrap()
}

/// Recorded `CodeDomain` observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodeDomain {
    /// Recorded input.
    pub input: CodeInput,
}
/// Recorded `CodeInput` observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodeInput {
    /// Recorded phase.
    pub phase: String,
    /// Recorded params.
    pub params: Value,
}
/// Read the selected-code value corpus.
/// # Panics
/// Panics if a committed fixture is malformed or the group is absent.
#[must_use]
pub fn code_domains() -> Vec<CodeDomain> {
    use crate::support::strict_record;
    let valid: Value =
        serde_json::from_str(include_str!("../fixtures/artifact_code_domains.json")).unwrap();
    strict_record::<CodeDomain>(&valid[0], &["input"]);
    strict_record::<CodeInput>(&valid[0]["input"], &["phase"]);
    serde_json::from_str(include_str!("../fixtures/artifact_code_domains.json")).unwrap()
}

#[cfg(target_arch = "wasm32")]
pub(crate) mod browser_records;

#[cfg(target_arch = "wasm32")]
/// Read direct diff title and line cases.
/// # Panics
/// Panics if a committed fixture is malformed or the group is absent.
#[must_use]
pub fn diff_cases() -> Vec<browser_records::DiffCase> {
    serde_json::from_str(include_str!("../fixtures/artifact_diff_views.json")).unwrap()
}
