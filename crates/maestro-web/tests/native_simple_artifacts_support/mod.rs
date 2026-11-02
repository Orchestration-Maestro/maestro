//! Recorded generic file artifact observations for the native browser bodies.
#![cfg(all(test, target_arch = "wasm32"))]
use serde::Deserialize;

/// Every recorded generic observation group.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Corpus {
    /// Localized literal guidance and headers.
    pub guidance: Vec<Guidance>,
    /// Filename to download media type.
    pub mime: Vec<MediaType>,
    /// Original content to decoded bytes or a rejected payload.
    pub decode: Vec<Decode>,
}
/// One localized guidance observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Guidance {
    /// Recorded operands.
    pub input: GuidanceInput,
    /// Recorded visible output.
    pub expected: GuidanceExpected,
}
/// Operands of one guidance observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuidanceInput {
    /// Authored filename.
    pub filename: String,
    /// Original base64 content.
    pub content: String,
    /// Selected language.
    pub language: String,
}
/// Visible output of one guidance observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuidanceExpected {
    /// Literal filename text.
    pub filename: String,
    /// The two guidance sentences in order.
    pub guidance: [String; 2],
    /// Header control tooltips.
    pub titles: Vec<String>,
}
/// One filename to media type observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MediaType {
    /// Authored filename.
    pub filename: String,
    /// Saved media type.
    pub mime: String,
}
/// One original content observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Decode {
    /// Original base64 content.
    pub content: String,
    /// Reached decoding result.
    pub expected: Decoded,
}
/// Reached decoding result.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, tag = "branch", rename_all = "snake_case")]
pub enum Decoded {
    /// Complete decoded bytes.
    Success {
        /// Decoded original bytes.
        bytes: Vec<u8>,
    },
    /// Rejected payload.
    Error,
}

/// Read the recorded generic corpus.
/// # Panics
/// Panics if the committed fixture is malformed.
#[must_use]
pub fn corpus() -> Corpus {
    serde_json::from_str(include_str!("../fixtures/native_simple_artifacts.json")).unwrap()
}
