//! Recorded invocation trees; each consumer compares complete expected observations.
#![cfg(test)]
use serde::Deserialize;
use serde_json::Value;

#[cfg(not(target_arch = "wasm32"))]
/// The four independently recorded invocation surfaces.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct Corpus {
    /// Registered callbacks, common preparation, ownership and guards.
    pub registered: Rows,
    /// Queued factories with hooks and failures.
    pub scripted: Rows,
    /// One retained queue observed by two sequential calls.
    pub retained_sequence: Sequence,
    /// Eager public operations and source factories.
    pub dispatch: Rows,
}
/// Named input/output scenarios.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Rows {
    /// All recorded inputs, without normalization.
    pub rows: Vec<Row>,
}
/// A recorded scenario.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Row {
    /// Selects and labels the public operation.
    pub name: String,
    /// Deserialized strictly by its operation's consumer.
    pub input: Value,
    /// Compared as a complete tree, including ordering and duplicates.
    pub expected: Value,
}
#[cfg(not(target_arch = "wasm32"))]
/// Sequential observations of one retained queue.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Sequence {
    /// The first and second invocation's complete observations.
    pub observations: Vec<Value>,
}
#[cfg(not(target_arch = "wasm32"))]
/// Load the byte-preserving, pretty-printed committed recording.
pub(super) fn load() -> Corpus {
    serde_json::from_str(include_str!("../fixtures/provider_affinity.json")).unwrap()
}
/// Hook combinations used by base preparation, with no unread input members.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct HookInput {
    /// Include a payload callback.
    pub payload: bool,
    /// Include a response callback.
    pub response: bool,
}

/// Read the portable hook-identity observations.
pub(super) fn hooks() -> Rows {
    serde_json::from_str(include_str!("../fixtures/provider_affinity_hooks.json")).unwrap()
}
