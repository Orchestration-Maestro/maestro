use maestro_models::{Tool, ToolCall, validate_tool_arguments};
use serde_json::{Value, json};

fn declaration(schema: Value) -> Tool {
    Tool {
        name: "count".into(),
        description: String::new(),
        parameters: schema,
    }
}

fn invocation(arguments: Value) -> Result<ToolCall, serde_json::Error> {
    Ok(ToolCall {
        id: "1".into(),
        name: "count".into(),
        arguments: serde_json::from_value(arguments)?,
        thought_signature: None,
    })
}

#[test]
fn maestro_validates_without_runtime_code_generation() {
    run_cases("maestro_validates_without_runtime_code_generation").unwrap();
}

#[test]
fn maestro_coerces_declared_primitive_types() {
    run_cases("maestro_coerces_declared_primitive_types").unwrap();
}

#[test]
fn maestro_rejects_invalid_primitive_coercion() {
    run_cases("maestro_rejects_invalid_primitive_coercion").unwrap();
}

#[test]
fn maestro_validation_selects_first_duplicate_tool() {
    run_cases("maestro_validation_selects_first_duplicate_tool").unwrap();
}

#[test]
fn maestro_reports_missing_tool_exactly() {
    run_cases("maestro_reports_missing_tool_exactly").unwrap();
}

#[test]
fn maestro_coerces_numeric_text_at_boundaries() {
    run_cases("maestro_coerces_numeric_text_at_boundaries").unwrap();
}

#[test]
fn maestro_formats_coerced_numbers_with_stable_spelling() {
    run_cases("maestro_formats_coerced_numbers_with_stable_spelling").unwrap();
}

#[test]
fn maestro_traverses_objects_items_and_additional_properties() {
    run_cases("maestro_traverses_objects_items_and_additional_properties").unwrap();
}

#[test]
fn maestro_validation_preserves_inputs_and_schema_changes() {
    run_cases("maestro_validation_preserves_inputs_and_schema_changes").unwrap();
    let mut tool = declaration(
        json!({"type":"object","properties":{"nested":{"type":"object","properties":{"count":{"type":"number","minimum":1}}}}}),
    );
    let call = invocation(json!({"nested":{"count":"2"}})).unwrap();
    let schema = tool.parameters.clone();
    let mut result = validate_tool_arguments(&tool, &call).unwrap();
    result["nested"]["count"] = json!(999);
    assert_eq!(call.arguments["nested"]["count"], "2");
    assert_eq!(tool.parameters, schema);
    tool.parameters["properties"]["nested"]["properties"]["count"]["minimum"] = json!(3);
    assert_eq!(
        validate_tool_arguments(&tool, &call).unwrap_err().message,
        "Validation failed for tool \"count\":\n  - nested.count: must be >= 3\n\nReceived arguments:\n{\n  \"nested\": {\n    \"count\": \"2\"\n  }\n}"
    );
    tool.parameters = json!({"type":"object","required":["missing"]});
    assert_eq!(
        validate_tool_arguments(&tool, &call).unwrap_err().message,
        "Validation failed for tool \"count\":\n  - missing: must have required properties missing\n\nReceived arguments:\n{\n  \"nested\": {\n    \"count\": \"2\"\n  }\n}"
    );
}

#[test]
fn maestro_counts_extended_graphemes_at_both_string_bounds() {
    run_cases("maestro_counts_extended_graphemes_at_both_string_bounds").unwrap();
}

#[test]
fn maestro_preserves_type_unions_but_tries_combinators_in_order() {
    run_cases("maestro_preserves_type_unions_but_tries_combinators_in_order").unwrap();
}

#[test]
fn maestro_reports_required_and_root_paths() {
    run_cases("maestro_reports_required_and_root_paths").unwrap();
}

#[test]
fn maestro_validates_pattern_and_prefix_constraints_without_extra_coercion() {
    run_cases("maestro_validates_pattern_and_prefix_constraints_without_extra_coercion").unwrap();
}

#[test]
fn maestro_validates_offline_refs_and_rejects_unknown_resources() {
    run_cases("maestro_validates_offline_refs_and_rejects_unknown_resources").unwrap();
}

#[derive(serde::Deserialize)]
struct Fixture {
    case: String,
    group: String,
    schema: Value,
    arguments: maestro_models::JsonObject,
    name: String,
    declaration: String,
    mode: FixtureMode,
    expected: Result<maestro_models::JsonObject, String>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum FixtureMode {
    Arguments,
    Missing,
    Duplicate,
}

fn run_cases(group: &str) -> Result<(), serde_json::Error> {
    let fixtures: Vec<Fixture> =
        serde_json::from_str(include_str!("fixtures/checked_arguments.json"))?;
    let cases: Vec<_> = fixtures
        .into_iter()
        .filter(|fixture| fixture.group == group)
        .collect();
    assert!(!cases.is_empty(), "missing fixture group {group}");
    for fixture in cases {
        run_case(fixture);
    }
    Ok(())
}

fn run_case(fixture: Fixture) {
    let tool = Tool {
        name: fixture.declaration,
        description: String::new(),
        parameters: fixture.schema,
    };
    let call = ToolCall {
        name: fixture.name,
        id: "fixture".into(),
        arguments: fixture.arguments,
        thought_signature: None,
    };
    let schema = tool.parameters.clone();
    let arguments = call.arguments.clone();
    let result = match fixture.mode {
        FixtureMode::Arguments => validate_tool_arguments(&tool, &call),
        FixtureMode::Duplicate => {
            let second = Tool {
                parameters: json!(false),
                ..tool.clone()
            };
            maestro_models::validate_tool_call(&[tool.clone(), second], &call)
        }
        FixtureMode::Missing if call.name == tool.name => {
            maestro_models::validate_tool_call(&[], &call)
        }
        FixtureMode::Missing => {
            maestro_models::validate_tool_call(std::slice::from_ref(&tool), &call)
        }
    };
    match (result, fixture.expected) {
        (Ok(actual), Ok(expected)) => assert!(
            equivalent(
                &Value::Object(actual.clone()),
                &Value::Object(expected.clone())
            ),
            "{}: actual {actual:?}, expected {expected:?}",
            fixture.case
        ),
        (Err(actual), Err(expected)) => {
            assert_eq!(actual.message, expected, "{}", fixture.case);
            assert_eq!(actual.name.as_deref(), Some("Error"));
            assert_eq!(actual.code, None);
            assert_eq!(actual.stack, None);
        }
        (actual, expected) => assert_eq!(
            actual.map_err(|error| error.message),
            expected,
            "{}",
            fixture.case
        ),
    }
    assert_eq!(tool.parameters, schema, "{}", fixture.case);
    assert_eq!(call.arguments, arguments, "{}", fixture.case);
}

fn equivalent(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Number(left), Value::Number(right)) => left.as_f64() == right.as_f64(),
        (Value::Array(left), Value::Array(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right)
                    .all(|(left, right)| equivalent(left, right))
        }
        (Value::Object(left), Value::Object(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .all(|(key, left)| right.get(key).is_some_and(|right| equivalent(left, right)))
        }
        _ => left == right,
    }
}

#[test]
fn maestro_validates_mixed_tuple_dialects_without_rewriting_literal_data() {
    run_cases("maestro_validates_mixed_tuple_dialects_without_rewriting_literal_data").unwrap();
}

#[test]
fn maestro_formats_original_arguments_in_error_order() {
    run_cases("maestro_formats_original_arguments_in_error_order").unwrap();
    let input = json!({"4294967295":4,"2":2,"01":1,"0":0,"4294967294":3,"-0":-0.0,"numeric":1e21,"unicode":"é\"\\\n"});
    let tool = declaration(json!(false));
    let error = validate_tool_arguments(&tool, &invocation(input).unwrap()).unwrap_err();
    assert_eq!(
        error.message,
        "Validation failed for tool \"count\":\n  - root: schema is false\n\nReceived arguments:\n{\n  \"0\": 0,\n  \"2\": 2,\n  \"4294967294\": 3,\n  \"4294967295\": 4,\n  \"01\": 1,\n  \"-0\": 0,\n  \"numeric\": 1e+21,\n  \"unicode\": \"é\\\"\\\\\\n\"\n}"
    );
}

#[test]
fn maestro_rejects_invalid_nested_scalar_candidates() {
    run_cases("maestro_rejects_invalid_nested_scalar_candidates").unwrap();
}

#[test]
fn maestro_preserves_original_schema_locations_for_tuple_references() {
    run_cases("maestro_preserves_original_schema_locations_for_tuple_references").unwrap();
}

#[test]
fn maestro_validates_both_tuple_forms_without_rewriting_literals() {
    run_cases("maestro_validates_both_tuple_forms_without_rewriting_literals").unwrap();
}

#[test]
fn maestro_asserts_registered_formats_and_ignores_unknown_formats() {
    run_cases("maestro_asserts_registered_formats_and_ignores_unknown_formats").unwrap();
}

#[test]
fn maestro_reports_invocation_names_and_reference_keyword_errors() {
    run_cases("maestro_reports_invocation_names_and_reference_keyword_errors").unwrap();
}
