//! Complete browser-owned card observations.
use super::{CodeInput, Filename, PhaseInput};
use serde::{Deserialize, Serialize};

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
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Expected {
    /// Recorded is custom.
    pub is_custom: bool,
    /// Recorded status.
    pub status: String,
    /// Recorded caption.
    pub caption: String,
    /// Recorded pill.
    pub pill: Option<String>,
    /// Recorded collapsible.
    pub collapsible: bool,
    /// Recorded initially collapsed.
    pub initially_collapsed: Option<bool>,
    /// Recorded code.
    pub code: Vec<CodeBlock>,
    /// Recorded console.
    pub console: Vec<Console>,
    /// Recorded errors.
    pub errors: Vec<String>,
    /// Recorded diff.
    pub diff: Vec<DiffRow>,
}
/// Recorded `CodeBlock` observations.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CodeBlock {
    /// Recorded code.
    pub code: String,
    /// Recorded language.
    pub language: String,
    /// Recorded decoded.
    pub decoded: String,
}
/// Recorded `Console` observations.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Console {
    /// Recorded content.
    pub content: String,
    /// Recorded variant.
    pub variant: String,
}
/// Recorded `DiffRow` observations.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DiffRow {
    /// Recorded text.
    pub text: String,
    /// Recorded kind.
    pub kind: String,
}
/// Recorded `Language` observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Language {
    /// Recorded input.
    pub input: Filename,
    /// Recorded expected.
    pub expected: String,
}
/// Recorded `Branch` observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, tag = "branch")]
pub enum Branch {
    /// Recorded rendered branch.
    #[serde(rename = "rendered")]
    Rendered { card: Expected },
    /// Recorded error branch.
    #[serde(rename = "error")]
    Error { kind: String },
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
/// Recorded `UnusedDiff` observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnusedDiff {
    /// Recorded input.
    pub input: crate::support::tool_input::Input,
    /// Recorded expected.
    pub expected: Branch,
}
/// Recorded `CodeDomain` observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodeDomain {
    /// Recorded input.
    pub input: CodeInput,
    /// Recorded expected.
    pub expected: CodeSelection,
}
/// Recorded `CodeSelection` observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, tag = "branch")]
pub enum CodeSelection {
    /// Recorded omitted branch.
    #[serde(rename = "omitted")]
    Omitted,
    /// Recorded text branch.
    #[serde(rename = "text")]
    Text { text: String },
}
/// Recorded `DiffCase` observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiffCase {
    /// Recorded input.
    pub input: DiffInput,
    /// Recorded expected.
    pub expected: DiffExpected,
}
/// Recorded `DiffInput` observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct DiffInput {
    /// Recorded old text.
    pub old_text: String,
    /// Recorded new text.
    pub new_text: String,
    /// Recorded title.
    pub title: Option<String>,
    /// Recorded class name.
    pub class_name: String,
}
/// Recorded `DiffExpected` observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct DiffExpected {
    /// Recorded rows.
    pub rows: Vec<DiffRow>,
    /// Recorded title.
    pub title: Option<String>,
    /// Recorded counts.
    pub counts: Option<String>,
    /// Recorded class present.
    pub class_present: bool,
}
