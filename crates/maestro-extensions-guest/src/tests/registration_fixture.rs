//! Typed registration inputs and independent expected observations.
use serde::{Deserialize, Serialize};

/// A Boolean or String flag payload, independently of its declared kind.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub(super) enum Value {
    /// A Boolean value, including false.
    Boolean(bool),
    /// A string value, including empty.
    String(String),
}

/// The declared parser kind.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum Kind {
    /// Boolean parsing.
    Boolean,
    /// String parsing.
    String,
}

/// The flag registration fields authored by the extension.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Flag {
    /// Literal name.
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
    /// Declared kind, not inferred from the default.
    #[serde(rename = "type")]
    pub kind: Kind,
    /// Independently typed optional default.
    pub default: Option<Value>,
}

/// Whether an authored callback awaits a host barrier.
#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum Mode {
    /// Ready without a host wait.
    Immediate,
    /// Suspends at a named host wait.
    Pending,
}

/// One candidate's independently selected fields.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Candidate {
    /// Inserted value.
    pub value: String,
    /// Display label, not derived from value.
    pub label: String,
    /// Optional description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Completion callback configuration.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Completion {
    /// Literal input supplied to the callback.
    pub prefix: String,
    /// Optional ordered answer.
    pub answer: Option<Vec<Candidate>>,
    /// Immediate or barrier-held future.
    pub mode: Mode,
}

/// The callback operation selected by a failure case.
#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum CallbackKind {
    /// Command handler.
    Command,
    /// Argument completion.
    Completion,
    /// Ordinary-context shortcut.
    Shortcut,
}

impl CallbackKind {
    /// The recorded operation name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Command => "command",
            Self::Completion => "completion",
            Self::Shortcut => "shortcut",
        }
    }
}

/// A callback failure with an independently controlled readiness phase.
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Failure {
    /// Selected callback kind.
    pub kind: CallbackKind,
    /// Whether the first call suspends.
    pub mode: Mode,
    /// Supplied exact error text.
    pub error: String,
}

/// Literal shortcut metadata and readiness configuration.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Shortcut {
    /// Literal key spelling.
    pub key: String,
    /// Optional description.
    pub description: Option<String>,
    /// Immediate or barrier-held invocation.
    pub mode: Mode,
}
