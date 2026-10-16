//! Strict complete original-byte recording.
#![cfg(test)]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
/// One recorded complete input with independent byte policies.
pub struct Row {
    /// Original string operand.
    pub input: Input,
    /// Complete artifact payload outcome.
    pub artifact: Outcome,
    /// Raw attachment outcome.
    pub overlay: Outcome,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
/// Original authored encoded content.
pub struct Input {
    /// Unmodified source string.
    pub content: String,
}
#[derive(serde::Deserialize)]
#[serde(tag = "branch", rename_all = "lowercase", deny_unknown_fields)]
/// Decoded original bytes or invalid-data failure.
pub enum Outcome {
    /// Accepted complete bytes.
    Success {
        /// Exact original byte sequence.
        bytes: Vec<u8>,
    },
    /// Invalid base64 input.
    Error,
}
/// Parse the strict recorded inputs.
/// # Panics
/// Panics if a committed row is malformed.
#[must_use]
pub fn rows() -> Vec<Row> {
    serde_json::from_str(include_str!("../fixtures/pdf_bytes.json")).unwrap()
}
