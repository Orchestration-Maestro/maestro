//! Executable schemas through the public argument validator.
#![cfg(test)]

#[path = "support/schema_effects.rs"]
mod schema_effects;

use maestro_models::{ParameterSchema, Tool, ToolCall, validate_tool_arguments};
use serde::Deserialize;
use serde_json::Value;

/// A complete source-observed application invocation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Stable input label.
    id: String,
    /// Schema with author-effect declarations.
    schema: Value,
    /// Original arguments.
    input: Value,
    /// Exact result or diagnostic.
    result: Expected,
}

/// Source-observed validation result.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "lowercase")]
enum Expected {
    /// Complete accepted value.
    Ok(Value),
    /// Exact error message.
    Error(String),
}

#[test]
fn converted_candidates_run_schema_checks_in_order() {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("fixtures/schema_phase_cases.json")).unwrap();
    assert_eq!(cases.len(), 126);
    for case in cases {
        let code = schema_effects::code(&case.schema);
        let tool = Tool {
            name: "relayed".into(),
            description: String::new(),
            parameters: ParameterSchema::<()>::from(case.schema).with_code(code),
        };
        let call = ToolCall {
            id: "call".into(),
            name: tool.name.clone(),
            arguments: case.input,
            thought_signature: None,
        };
        let result = validate_tool_arguments(&tool, &call);
        assert_result(&case.id, result, case.result);
    }
}

/// One source-observed executable callback program.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CallbackCase {
    /// Exact source input label.
    id: String,
    /// Owning behavior group.
    test: String,
    /// Original arguments.
    input: Value,
    /// Complete result or error.
    result: Expected,
}

/// Replay one callback group through the public validator.
fn callback_cases(test: &str) {
    let cases: Vec<CallbackCase> =
        serde_json::from_str(include_str!("fixtures/executable_schema_cases.json")).unwrap();
    let mut selected = 0;
    for case in cases.into_iter().filter(|case| case.test == test) {
        selected += 1;
        let refinements = refinements(&case.id);
        let code = std::collections::HashMap::from([(
            "/properties/n".into(),
            maestro_models::SchemaNode {
                base: None,
                refinements,
            },
        )]);
        let tool = Tool {
            name: "custom".into(),
            description: String::new(),
            parameters: ParameterSchema::<()>::from(serde_json::json!({
                "type": "object", "properties": { "n": { "type": "number" } }, "required": ["n"],
            }))
            .with_code(code),
        };
        let call = ToolCall {
            id: "call".into(),
            name: tool.name.clone(),
            arguments: case.input,
            thought_signature: None,
        };
        assert_result(&case.id, validate_tool_arguments(&tool, &call), case.result);
    }
    assert!(selected > 0);
}

/// Construct the same author callbacks as each recorded source program.
fn refinements(id: &str) -> Vec<maestro_models::Refinement> {
    use maestro_models::{Refinement, SchemaCallback};
    let count = if matches!(id, "many-refinement-messages" | "error-factory-after-limit") {
        10
    } else if id.starts_with("refine-stack-") || id == "equal-refinement-messages" {
        2
    } else {
        1
    };
    (0..count)
        .map(|index| {
            let accepted = (id == "refine-stack-first-ok" && index == 0)
                || (id == "refine-stack-second-ok" && index == 1);
            let id = id.to_owned();
            Refinement {
                check: SchemaCallback::new(move |_| Ok(accepted)),
                error: SchemaCallback::new(move |value| refinement_message(&id, index, value)),
            }
        })
        .collect()
}

/// Render the author program's error text with canonical number spelling.
fn refinement_message(
    id: &str,
    index: usize,
    value: &Value,
) -> Result<String, maestro_models::DiagnosticErrorInfo> {
    Ok(match id {
        "refine-default-message" => "Refine Error".into(),
        "refine-static-message" => "not admitted".into(),
        "refine-dynamic-message" => format!(
            "not admitted: {}",
            maestro_json::compact_json(value).unwrap()
        ),
        "refine-empty-errors" => String::new(),
        "equal-refinement-messages" => "same".into(),
        "error-factory-after-limit" if index == 9 => {
            return Err(maestro_models::DiagnosticErrorInfo::error(
                "last factory stopped",
            ));
        }
        "many-refinement-messages" | "error-factory-after-limit" => format!("error {index}"),
        "refine-stack-errors" | "refine-stack-first-ok" | "refine-stack-second-ok" => {
            ["first", "second"][index].into()
        }
        _ => panic!("unknown callback program"),
    })
}

/// Compare the complete canonical value or the exact diagnostic envelope.
fn assert_result(
    id: &str,
    result: Result<Value, maestro_models::DiagnosticErrorInfo>,
    expected: Expected,
) {
    match expected {
        Expected::Ok(expected) => assert_eq!(
            maestro_json::compact_json(&result.unwrap()).unwrap(),
            maestro_json::compact_json(&expected).unwrap(),
            "{id}"
        ),
        Expected::Error(expected) => assert_eq!(result.unwrap_err().message, expected, "{id}"),
    }
}

#[test]
fn stacked_refinements_keep_order_and_all_failures() {
    callback_cases("stacked_refinements_keep_order_and_all_failures");
}

#[test]
fn refinement_messages_keep_default_static_and_dynamic_text() {
    callback_cases("refinement_messages_keep_default_static_and_dynamic_text");
}

#[test]
fn refinement_errors_keep_distinct_indices_and_the_error_limit() {
    callback_cases("refinement_errors_keep_distinct_indices_and_the_error_limit");
}
