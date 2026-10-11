//! Source-recorded inputs with strict field consumption.
use super::registration_fixture::{Kind, Value};
use serde::Deserialize;

/// An input with its independently recorded expected observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Case<I, E> {
    /// Operation input.
    pub input: I,
    /// Recorded result.
    pub expected: E,
}

/// One flag/default/override combination.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FlagInput {
    /// Literal name.
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
    /// Declared parser kind.
    #[serde(rename = "type")]
    pub kind: Kind,
    /// Optional default.
    pub default: Option<Value>,
    /// Supplied override before registration.
    #[serde(rename = "override")]
    pub override_value: Option<Value>,
}

/// Recorded registration metadata and current value.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FlagExpected {
    /// Read before registering the name.
    #[serde(default)]
    pub unknown: Option<Value>,
    /// Recorded description.
    pub description: Option<String>,
    /// Recorded kind.
    #[serde(rename = "type")]
    pub kind: Kind,
    /// Recorded default.
    pub default: Option<Value>,
    /// Host-resolved current value.
    pub value: Option<Value>,
}

/// Flag-only corpus used by the flag transport witness.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FlagCorpus {
    /// All independent kind/default/override combinations.
    pub flags: Vec<Case<FlagInput, FlagExpected>>,
    /// Command option combinations.
    pub commands: Vec<Case<Command, Command>>,
    /// Shortcut keys, descriptions, origins and readiness.
    pub shortcuts: Vec<Case<ShortcutInput, ShortcutExpected>>,
    /// Each callback kind's immediate and pending failure.
    #[serde(rename = "callbackErrors")]
    pub callback_errors: Vec<Case<super::registration_fixture::Failure, FailureExpected>>,
    /// Literal prefixes crossed with outcomes and future readiness.
    pub completions: Vec<Case<super::registration_fixture::Completion, CompletionExpected>>,
    /// The documented filtering example.
    pub example: Vec<Case<String, Option<Vec<super::registration_fixture::Candidate>>>>,
    /// Literal names with distinguishable decoy replies.
    #[serde(rename = "literalFlags")]
    pub literal_flags: Vec<LiteralFlag>,
    /// Replacement defaults with the source's surviving value.
    pub replacements: Vec<Case<Replacement, FlagExpected>>,
    /// Host changes after registration.
    #[serde(rename = "liveReads")]
    pub live_reads: Vec<Case<Value, Vec<Option<Value>>>>,
}

/// A literal flag name and independent decoy observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LiteralFlag {
    /// Exact selected name.
    pub input: String,
    /// Value of that name.
    pub expected: Value,
    /// Value of a distinct name.
    pub decoy: Option<Value>,
}

/// Two registrations of the same name.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Replacement {
    /// First default.
    pub first: Option<Value>,
    /// Replacement default.
    pub second: Option<Value>,
}

/// The prefix the callback received and its ordered answer.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CompletionExpected {
    /// Received prefix.
    pub prefix: String,
    /// Optional candidate list.
    pub answer: Option<Vec<super::registration_fixture::Candidate>>,
}

/// The supplied error and effects observed before it.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FailureExpected {
    /// Exact failure message.
    pub error: String,
    /// Ordered earlier effects.
    pub effects: Vec<String>,
}

/// Command fields independently observed by the registration host.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Command {
    /// Literal name.
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
    /// Independent optional callback presence.
    pub completion: bool,
}

/// Selected shortcut with its invoked context origin.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ShortcutInput {
    /// Literal key spelling.
    pub key: String,
    /// Optional description.
    pub description: Option<String>,
    /// Immediate or pending invocation.
    pub mode: super::registration_fixture::Mode,
    /// Invoked ordinary context's directory.
    pub cwd: String,
}

/// Host-observed shortcut metadata and callback origin.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ShortcutExpected {
    /// Literal registered spelling.
    pub key: String,
    /// Optional description.
    pub description: Option<String>,
    /// Actual callback directory.
    pub cwd: String,
}
