//! Recorded classification, localization and content operands for native overlay observations.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::attachment_support::Input;
use serde::Deserialize;
/// Ordered classification case.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TypeCase {
    /// Case identity.
    pub(crate) name: String,
    /// Original attachment, absent only before opening.
    pub(crate) input: Input,
    /// Expected selected kind.
    pub(crate) expected: TypeExpected,
}
/// Classifier output.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TypeExpected {
    /// Source kind.
    pub(crate) r#type: String,
}
/// Localized toggle input and complete mode labels.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ToggleCase {
    /// Case identity.
    pub(crate) name: String,
    /// Original record.
    pub(crate) input: Input,
    /// Selected language.
    pub(crate) language: String,
    /// Exact mode labels/index.
    pub(crate) expected: ToggleExpected,
}
/// Source toggle output.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ToggleExpected {
    /// Preview label followed by text label.
    pub(crate) modes: Vec<String>,
    /// Initial selected index, absent with no toggle.
    pub(crate) selected: Option<usize>,
}
/// Source content selection input/output.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ContentCase {
    /// Case identity.
    pub(crate) name: String,
    /// Typed original record.
    pub(crate) input: Input,
    /// Whether text mode was selected.
    pub(crate) show_text: bool,
    /// Full expected content projection.
    pub(crate) expected: ContentExpected,
}
/// Source content result.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ContentExpected {
    /// Selected content kind.
    pub(crate) kind: String,
    /// Original image URL.
    pub(crate) source: Option<String>,
    /// Literal text, absent for document containers.
    pub(crate) text: Option<String>,
    /// Extracted-text font/layout selection.
    pub(crate) small: bool,
}
/// Source-derived content cases.
pub(crate) fn content_cases() -> Vec<ContentCase> {
    serde_json::from_str(include_str!("fixtures/attachment_content.json")).unwrap()
}
