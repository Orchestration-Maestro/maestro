//! Opt-in helpers for controlled tests: the request owner's records and the recorded
//! tool-argument conversion corpus.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::{Map, Value, json};

pub use maestro_request::test_support::*;

use crate::{ParameterSchema, SchemaCodeSource, SchemaKind, Tool, ToolCall};

/// The recorded tool-argument conversion corpus.
const CORPUS: &str = include_str!("../tests/fixtures/tool_argument_conversion.json");

/// One recorded case of the conversion corpus.
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProducerCase {
    /// Case label; a producer case is labelled with its declaring tool's name.
    #[serde(rename = "case")]
    label: String,
    /// Recorded group; producer cases hold tool declarations.
    group: String,
    /// Supplied schema value.
    pub schema: Value,
    /// Declared node kinds by JSON pointer.
    pub kinds: BTreeMap<String, SchemaKind>,
    /// Recorded original arguments.
    pub arguments: Map<String, Value>,
    /// Recorded prepared arguments or diagnostic.
    pub expected: Value,
}

/// The recorded producer case for the tool `name`.
///
/// # Panics
/// Panics unless the corpus parses and holds exactly one producer case for `name`, with a
/// schema, declared kinds and arguments.
#[must_use]
#[track_caller]
pub fn producer_case(name: &str) -> ProducerCase {
    let parsed = serde_json::from_str::<Vec<ProducerCase>>(CORPUS);
    assert!(
        parsed.is_ok(),
        "recorded conversion corpus: {:?}",
        parsed.as_ref().err()
    );
    let mut selected: Vec<ProducerCase> = parsed
        .unwrap_or_default()
        .into_iter()
        .filter(|case| case.group == "producer" && case.label == name)
        .collect();
    assert_eq!(selected.len(), 1, "one recorded producer case for {name}");
    let case = selected.pop().unwrap_or_default();
    assert!(!case.schema.is_null());
    assert!(!case.kinds.is_empty());
    assert!(!case.arguments.is_empty());
    case
}

/// Assert one declaration keeps the recorded JSON, node kinds and prepared outcome.
///
/// # Panics
/// Panics when the declaration differs from the producer case [`producer_case`] reads.
#[track_caller]
pub fn assert_declaration<C: SchemaCodeSource + Clone + Default>(
    name: &str,
    parameters: &ParameterSchema<C>,
) {
    let case = producer_case(name);
    assert_eq!(parameters.as_json(), &case.schema, "{name} schema");
    assert_eq!(
        parameters.conversions().len(),
        case.kinds.len(),
        "{name} kinds"
    );
    for (pointer, kind) in &case.kinds {
        assert_eq!(
            parameters.conversions().get(pointer),
            Some(kind),
            "{name} kind at {pointer:?}"
        );
    }
    let tool = Tool {
        name: name.to_owned(),
        description: String::new(),
        parameters: parameters.clone(),
    };
    let call = ToolCall {
        id: "call".to_owned(),
        name: name.to_owned(),
        arguments: Value::Object(case.arguments),
        thought_signature: None,
    };
    let outcome = match crate::validate_tool_arguments(&tool, &call) {
        Ok(arguments) => json!({"Ok": arguments}),
        Err(error) => json!({"Err": error.message}),
    };
    assert_eq!(outcome, case.expected, "{name} prepared outcome");
}
