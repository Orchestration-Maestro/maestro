//! Validation keeps the complete JSON root of a tool call's arguments.
use std::collections::BTreeMap;

use crate::json_equivalence::equivalent;
use maestro_models::{
    ParameterSchema, SchemaKind, Tool, ToolCall, validate_tool_arguments, validate_tool_call,
};
use serde_json::{Value, json};

/// One recorded schema family and operand with its complete outcome.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    /// Schema family the case belongs to.
    kind: String,
    /// Whether the schema carries authored kinds.
    authored: bool,
    /// Supplied schema value.
    schema: Value,
    /// Declared node kinds by JSON pointer.
    kinds: BTreeMap<String, SchemaKind>,
    /// Original argument root.
    input: Value,
    /// Recorded outcome.
    expected: Expected,
}

/// The recorded outcome of validating one case.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "lowercase")]
enum Expected {
    /// Accepted arguments.
    Ok(Value),
    /// Exact diagnostic text.
    Error(String),
}

/// The recorded corpus of complete validation queries; the parser section belongs to its own
/// reader.
#[derive(serde::Deserialize)]
struct Corpus {
    /// Validation rows.
    validation: Vec<Row>,
}

/// Build the declaration one row supplies.
fn declaration(row: &Row) -> Tool {
    let parameters = if row.authored {
        ParameterSchema::authored(row.schema.clone(), row.kinds.clone())
    } else {
        ParameterSchema::from(row.schema.clone())
    };
    Tool {
        name: "root".into(),
        description: String::new(),
        parameters,
    }
}

/// Build the invocation one row supplies.
fn invocation(row: &Row) -> ToolCall {
    ToolCall {
        id: "call".into(),
        name: "root".into(),
        arguments: row.input.clone(),
        thought_signature: None,
    }
}

/// Every recorded family, operand and origin keeps its complete outcome.
#[test]
fn validation_preserves_recorded_root_outcomes() {
    let corpus: Corpus = serde_json::from_str(include_str!("fixtures/tool_argument_roots.json"))
        .expect("the recorded corpus decodes");
    assert_eq!(corpus.validation.len(), 630);
    let families: std::collections::BTreeSet<&str> = corpus
        .validation
        .iter()
        .map(|row| row.kind.as_str())
        .collect();
    assert_eq!(families.len(), 23);
    for row in &corpus.validation {
        let tool = declaration(row);
        let call = invocation(row);
        let actual = validate_tool_arguments(&tool, &call).map_err(|error| error.message);
        match (&actual, &row.expected) {
            (Ok(actual), Expected::Ok(expected)) => assert!(
                equivalent(actual, expected),
                "{} {:?}: accepted {actual:?}, recorded {expected:?}",
                row.kind,
                row.input
            ),
            (Err(actual), Expected::Error(expected)) => {
                assert_eq!(actual, expected, "{} {:?}", row.kind, row.input);
            }
            _ => panic!(
                "{} {:?}: outcome {actual:?}, recorded {:?}",
                row.kind, row.input, row.expected
            ),
        }
        assert_eq!(call.arguments, row.input, "{}", row.kind);
        assert_eq!(tool.parameters.as_json(), &row.schema, "{}", row.kind);
    }
}

/// Both public validation operations report the original root, in order.
#[test]
fn root_validation_keeps_original_diagnostics() {
    let text_tool: Tool = Tool {
        name: "text".into(),
        description: String::new(),
        parameters: json!({"type": "object"}).into(),
    };
    let call = |name: &str, arguments: Value| ToolCall {
        id: "call".into(),
        name: name.into(),
        arguments,
        thought_signature: None,
    };

    assert_eq!(
        validate_tool_call::<()>(&[], &call("missing", json!({"q": 1})))
            .expect_err("an unknown tool is rejected")
            .message,
        "Tool \"missing\" not found"
    );
    assert_eq!(
        validate_tool_call(
            std::slice::from_ref(&text_tool),
            &call("text", json!({"q": 1}))
        )
        .expect("a matching declaration is selected"),
        json!({"q": 1})
    );

    for input in [
        json!([1]),
        json!(false),
        Value::Null,
        json!("text"),
        json!(3),
    ] {
        let rendering = match &input {
            Value::String(text) => format!("\"{text}\""),
            other => serde_json::to_string_pretty(other).expect("the input renders"),
        };
        let error = validate_tool_arguments(&text_tool, &call("text", input))
            .expect_err("a non-object root does not satisfy an object schema")
            .message;
        assert_eq!(
            error,
            format!(
                "Validation failed for tool \"text\":\n  - root: must be object\n\nReceived arguments:\n{rendering}"
            )
        );
    }

    let number: Tool = Tool {
        name: "number".into(),
        description: String::new(),
        parameters: json!({"type": "number", "minimum": 100}).into(),
    };
    let error = validate_tool_arguments(&number, &call("number", json!("42")))
        .expect_err("a converted root that fails its schema is rejected")
        .message;
    assert_eq!(
        error,
        "Validation failed for tool \"number\":\n  - root: must be >= 100\n\nReceived arguments:\n\"42\""
    );
}
