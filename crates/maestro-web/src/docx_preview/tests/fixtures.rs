//! Strict operation envelopes for the recorded stylesheet corpus.
use serde::Deserialize;
use serde_json::Value;

/// Declaration occurrences before duplicate-key insertion.
pub(super) type Pairs = Vec<(String, Option<String>)>;

/// One complete recorded operation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Case {
    /// Behavior owning this input.
    pub test: String,
    /// Complete operation arguments.
    pub input: Input,
    /// Recorded completion kind.
    pub outcome: String,
    /// Complete independent observation.
    pub expected: Value,
}

/// Arguments admitted by each recorded operation, with no unused members.
#[derive(Deserialize)]
#[serde(
    tag = "op",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub(super) enum Input {
    /// Literal class conversion.
    Class {
        /// Authored identifier.
        value: String,
    },
    /// Retained namespace and style identifier.
    Prefix {
        /// Nullable identifier.
        value: Option<String>,
        /// Authored namespace.
        prefix: String,
    },
    /// Raw rule emission.
    Css {
        /// Complete selector text.
        selectors: String,
        /// Ordered declaration occurrences.
        values: Pairs,
        /// Literal optional CSS suffix.
        suffix: Option<String>,
    },
    /// Missing-declaration copying.
    Copy {
        /// Optional source declarations.
        input: Option<Pairs>,
        /// Optional existing destination.
        output: Option<Pairs>,
        /// Optional ordered key selection.
        attrs: Option<Vec<String>>,
    },
    /// Owned definitions supplied directly.
    Styles {
        /// Complete unresolved style records.
        styles: Vec<Value>,
    },
    /// Constructor forwarding to all stylesheet surfaces.
    Namespace {
        /// Authored namespace.
        class_name: String,
        /// Complete unresolved style records.
        styles: Vec<Value>,
    },
    /// Default document CSS.
    Default {
        /// Print wrapper suppression policy.
        hide_wrapper_on_print: bool,
    },
    /// Trusted child markup built from text nodes.
    Wrapper {
        /// Authored namespace.
        class_name: String,
        /// Ordered literal text children.
        children: Vec<String>,
    },
    /// Trusted already-rendered subtree.
    WrapperHtml {
        /// Authored namespace.
        class_name: String,
        /// Complete child markup.
        children_html: String,
    },
    /// Real parsed-document artifact.
    Parsed {
        /// Complete styles XML.
        xml: String,
        /// Cell-width policy.
        ignore_width: bool,
    },
    /// Parsed definitions and retained-state lookup queries.
    Inheritance {
        /// Complete styles XML.
        xml: String,
        /// Cell-width policy.
        ignore_width: bool,
        /// Exact nullable lookup names in encounter order.
        lookups: Vec<Option<String>>,
    },
}

/// Load the complete independently recorded input/output envelope.
pub(super) fn cases() -> Vec<Case> {
    serde_json::from_str(include_str!("../fixtures/styles.json")).unwrap()
}
