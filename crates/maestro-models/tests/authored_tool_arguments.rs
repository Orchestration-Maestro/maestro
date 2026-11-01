//! Prepared tool arguments for authored and plain parameter schemas.
//!
//! Every recorded case supplies a schema, the kinds its author declared for individual nodes,
//! the original arguments and the prepared arguments or the exact diagnostic. The recorded
//! outcomes come from the reference validator; the record rows record the matching that
//! validator applies to a member name, which conversion follows as it is written.

use std::collections::BTreeMap;
use std::collections::HashSet;

use crate::json_equivalence::equivalent;
use maestro_models::{
    ParameterSchema, SchemaKind, Tool, ToolCall, validate_tool_arguments, validate_tool_call,
};
use serde_json::{Map, Value};

/// One recorded case.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Case label, unique across the corpus.
    #[serde(rename = "case")]
    label: String,
    /// Group the case partitions into.
    group: String,
    /// Supplied schema value.
    schema: Value,
    /// Declared node kinds by JSON pointer.
    kinds: BTreeMap<String, SchemaKind>,
    /// Original arguments.
    arguments: Value,
    /// Prepared arguments or the exact diagnostic text.
    expected: Expected,
}

/// The recorded outcome of preparing one case.
#[derive(Debug, serde::Deserialize, PartialEq)]
enum Expected {
    /// Prepared arguments.
    Ok(Value),
    /// Exact diagnostic text.
    Err(String),
}

/// Cases each named test asserts, by group name or case label.
const MATRIX: &[(&str, &[&str])] = &[
    ("string_inputs_preserve_schema_origin", &["group:string"]),
    ("number_inputs_preserve_schema_origin", &["group:number"]),
    ("integer_inputs_preserve_schema_origin", &["group:integer"]),
    ("boolean_inputs_preserve_schema_origin", &["group:boolean"]),
    ("null_inputs_preserve_schema_origin", &["group:null"]),
    ("array_inputs_preserve_schema_origin", &["group:array"]),
    ("object_inputs_preserve_schema_origin", &["group:object"]),
    (
        "mixed_nodes_preserve_conversion_boundaries",
        &["mixed children", "plain parent authored child"],
    ),
    (
        "nested_collections_convert_present_members",
        &[
            "authored array strings",
            "number array wraps fractional text",
            "nested object leaves",
        ],
    ),
    (
        "tuple_and_prefix_items_keep_separate_routes",
        &[
            "plain tuple children",
            "authored tuple children",
            "prefix items remain plain",
        ],
    ),
    (
        "additional_properties_convert_only_the_complement",
        &[
            "additional declared and extra",
            "additional empty properties",
        ],
    ),
    (
        "property_names_are_exact_keys",
        &["literal property identity", "regex-looking property"],
    ),
    (
        "plain_compositions_keep_their_existing_phase",
        &[
            "allof JSON branch remains plain",
            "anyof JSON branch remains plain",
            "type union JSON remains plain",
        ],
    ),
    (
        "union_candidates_keep_phase_and_declaration_order",
        &[
            "authored union preserves matching",
            "authored union converts",
            "plain union first convert",
        ],
    ),
    (
        "missing_members_are_not_defaulted",
        &[
            "optional absent not defaulted",
            "missing required not inserted",
            "unknown properties retained",
        ],
    ),
    (
        "validation_keeps_original_arguments_and_exact_errors",
        &[
            "validation rejects after conversion",
            "conversion no input mutation",
        ],
    ),
    (
        "numeric_text_uses_stable_scalar_spelling",
        &["group:numbers"],
    ),
    (
        "record_patterns_share_validator_matching",
        &[
            "group:patterns",
            "record",
            "record_mixed",
            "record_pattern",
            "record overlapping patterns and extra",
        ],
    ),
    (
        "literal_and_enum_conversion_keeps_declared_order",
        &[
            "literal_string",
            "literal_number",
            "literal_boolean",
            "literal_rejected",
            "literal_mixed",
            "enum",
            "enum_reverse",
            "enum_mixed",
        ],
    ),
    (
        "intersections_keep_narrowing_and_diagnostic_order",
        &[
            "intersect_object",
            "intersect_mixed",
            "intersect_same_key",
            "intersect_scalar",
            "intersect_array",
            "intersect disjoint kinds retain diagnostics",
            "intersect narrow literal first",
            "intersect narrow literal second",
            "intersect union overlap",
            "intersect empty members",
        ],
    ),
    (
        "references_keep_scope_and_plain_boundaries",
        &[
            "ref",
            "ref_mixed",
            "ref_recursive",
            "ref_absent_context",
            "ref_local_defs_plain",
        ],
    ),
    (
        "template_literals_keep_finite_and_infinite_routes",
        &[
            "template_literal",
            "template_null",
            "template_reject",
            "template_mixed",
            "template finite mismatch keeps final error phase",
            "template finite alternatives convert",
            "template empty literal",
            "cyclic finite template keeps original kind",
        ],
    ),
    (
        "unsafe_nodes_remain_plain",
        &["unsafe", "root_plain_object"],
    ),
    (
        "nested_intersection_keeps_child_conversions",
        &["intersect_nested_object", "intersect_nested_array"],
    ),
    (
        "intersection_narrowing_keeps_the_declared_operand",
        &["intersect_unknown_left", "intersect_unknown_right"],
    ),
    (
        "template_alternatives_stay_alternatives",
        &[
            "template alternatives null first",
            "template alternatives true first",
        ],
    ),
    (
        "nested_definitions_restore_the_enclosing_scope",
        &["cyclic_scope"],
    ),
    (
        "comparison_keeps_each_kind_and_modifier_separate",
        &["group:comparison"],
    ),
];

/// Read the recorded corpus.
fn corpus() -> serde_json::Result<Vec<Case>> {
    serde_json::from_str(include_str!("fixtures/tool_argument_conversion.json"))
}

/// Read the recorded corpus, failing the calling test when it does not parse.
#[track_caller]
fn recorded() -> Vec<Case> {
    let parsed = corpus();
    assert!(parsed.is_ok(), "recorded conversion corpus");
    parsed.unwrap_or_default()
}

/// Assert every case the supplied test name owns.
#[track_caller]
fn run(test: &str) {
    let cases = selected(test);
    assert!(!cases.is_empty(), "test {test} owns no recorded case");
    for case in &cases {
        assert_case(case);
    }
}

/// Select the cases one named test owns.
fn selected(test: &str) -> Vec<Case> {
    let selectors = MATRIX
        .iter()
        .find(|(name, _)| *name == test)
        .map_or(&[][..], |(_, selectors)| *selectors);
    assert!(
        !selectors.is_empty(),
        "test {test} is not in the case partition"
    );
    recorded()
        .into_iter()
        .filter(|case| {
            selectors
                .iter()
                .any(|selector| match selector.strip_prefix("group:") {
                    Some(group) => case.group == group,
                    None => case.label == *selector,
                })
        })
        .collect()
}

/// Assert one recorded case: preserved inputs and the exact prepared outcome.
#[track_caller]
fn assert_case(case: &Case) {
    let schema: ParameterSchema =
        ParameterSchema::authored(case.schema.clone(), case.kinds.clone());
    let tool = Tool {
        name: "echo".to_owned(),
        description: "Recorded case".to_owned(),
        parameters: schema,
    };
    let call = ToolCall {
        id: "call".to_owned(),
        name: "echo".to_owned(),
        arguments: case.arguments.clone(),
        thought_signature: None,
    };
    let prepared = if case.label == "tool selection public route" {
        validate_tool_call(std::slice::from_ref(&tool), &call).map_err(|error| error.message)
    } else {
        validate_tool_arguments(&tool, &call).map_err(|error| error.message)
    };
    let outcome = match prepared {
        Ok(arguments) => Expected::Ok(arguments),
        Err(message) => Expected::Err(message),
    };
    assert!(
        same_outcome(&outcome, &case.expected),
        "case {}: prepared {outcome:?}, recorded {:?}",
        case.label,
        case.expected
    );
    assert_eq!(call.arguments, case.arguments, "case {}", case.label);
    assert_eq!(
        tool.parameters.as_json(),
        &case.schema,
        "case {}",
        case.label
    );
}

/// Compare outcomes, treating numbers by their binary64 value.
fn same_outcome(actual: &Expected, expected: &Expected) -> bool {
    match (actual, expected) {
        (Expected::Err(actual), Expected::Err(expected)) => actual == expected,
        (Expected::Ok(actual), Expected::Ok(expected)) => equivalent(actual, expected),
        _ => false,
    }
}

/// Every recorded case is owned by exactly one named test.
#[test]
fn every_recorded_case_is_asserted_once() {
    let mut seen = HashSet::new();
    for (test, _) in MATRIX {
        for case in selected(test) {
            assert!(
                seen.insert(case.label.clone()),
                "case {} is owned twice",
                case.label
            );
        }
    }
    let recorded = recorded();
    let unowned: Vec<&Case> = recorded
        .iter()
        .filter(|case| !seen.contains(&case.label))
        .collect();
    assert_eq!(
        unowned.len(),
        10,
        "only the bundled and browser descriptor suites own recorded producer cases"
    );
    assert!(unowned.iter().all(|case| case.group == "producer"));
}

/// Text parameters keep their authored conversion and their plain conversion.
#[test]
fn string_inputs_preserve_schema_origin() {
    run("string_inputs_preserve_schema_origin");
}

/// Number parameters accept the recorded text and scalar spellings.
#[test]
fn number_inputs_preserve_schema_origin() {
    run("number_inputs_preserve_schema_origin");
}

/// Integer parameters truncate authored values and reject plain fractions.
#[test]
fn integer_inputs_preserve_schema_origin() {
    run("integer_inputs_preserve_schema_origin");
}

/// Boolean parameters convert the authored words and the narrow plain rules.
#[test]
fn boolean_inputs_preserve_schema_origin() {
    run("boolean_inputs_preserve_schema_origin");
}

/// Null parameters convert the authored words and the plain empty scalars.
#[test]
fn null_inputs_preserve_schema_origin() {
    run("null_inputs_preserve_schema_origin");
}

/// Array parameters wrap authored scalars and leave plain schemas unwrapped.
#[test]
fn array_inputs_preserve_schema_origin() {
    run("array_inputs_preserve_schema_origin");
}

/// Object parameters convert present members only.
#[test]
fn object_inputs_preserve_schema_origin() {
    run("object_inputs_preserve_schema_origin");
}

/// Mixed authored and plain children keep their separate results.
#[test]
fn mixed_nodes_preserve_conversion_boundaries() {
    run("mixed_nodes_preserve_conversion_boundaries");
}

/// Nested collections convert through their own declared nodes.
#[test]
fn nested_collections_convert_present_members() {
    run("nested_collections_convert_present_members");
}

/// Tuples convert corresponding positions; prefix items do not.
#[test]
fn tuple_and_prefix_items_keep_separate_routes() {
    run("tuple_and_prefix_items_keep_separate_routes");
}

/// Additional members convert the complement of the declared entries.
#[test]
fn additional_properties_convert_only_the_complement() {
    run("additional_properties_convert_only_the_complement");
}

/// Declared property names are literal keys.
#[test]
fn property_names_are_exact_keys() {
    run("property_names_are_exact_keys");
}

/// Plain compositions keep their existing phase.
#[test]
fn plain_compositions_keep_their_existing_phase() {
    run("plain_compositions_keep_their_existing_phase");
}

/// Union alternatives keep their phase and declaration order.
#[test]
fn union_candidates_keep_phase_and_declaration_order() {
    run("union_candidates_keep_phase_and_declaration_order");
}

/// Absent members stay absent and required members stay required.
#[test]
fn missing_members_are_not_defaulted() {
    run("missing_members_are_not_defaulted");
}

/// Failed preparation reports the original arguments and exact diagnostics.
#[test]
fn validation_keeps_original_arguments_and_exact_errors() {
    run("validation_keeps_original_arguments_and_exact_errors");
}

/// Numeric text uses the stable scalar spelling.
#[test]
fn numeric_text_uses_stable_scalar_spelling() {
    run("numeric_text_uses_stable_scalar_spelling");
}

/// Record values match the validator's own pattern search.
#[test]
fn record_patterns_share_validator_matching() {
    run("record_patterns_share_validator_matching");
}

/// Literals and enumerations keep their declared order.
#[test]
fn literal_and_enum_conversion_keeps_declared_order() {
    run("literal_and_enum_conversion_keeps_declared_order");
}

/// Intersections narrow before converting and keep the diagnostic order.
#[test]
fn intersections_keep_narrowing_and_diagnostic_order() {
    run("intersections_keep_narrowing_and_diagnostic_order");
}

/// References keep their scope and their plain boundaries.
#[test]
fn references_keep_scope_and_plain_boundaries() {
    run("references_keep_scope_and_plain_boundaries");
}

/// Template patterns keep their finite and unbounded routes.
#[test]
fn template_literals_keep_finite_and_infinite_routes() {
    run("template_literals_keep_finite_and_infinite_routes");
}

/// Untagged schemas and untagged nested roots stay plain.
#[test]
fn unsafe_nodes_remain_plain() {
    run("unsafe_nodes_remain_plain");
}

/// Members an intersection merges keep the conversions declared for their own nodes.
#[test]
fn nested_intersection_keeps_child_conversions() {
    run("nested_intersection_keeps_child_conversions");
}

/// Narrowing an unknown member keeps the declared operand, whichever side it is on.
#[test]
fn intersection_narrowing_keeps_the_declared_operand() {
    run("intersection_narrowing_keeps_the_declared_operand");
}

/// Template alternatives convert through every alternative, not their concatenation.
#[test]
fn template_alternatives_stay_alternatives() {
    run("template_alternatives_stay_alternatives");
}

/// A nested definition block leaves the enclosing definitions visible again.
#[test]
fn nested_definitions_restore_the_enclosing_scope() {
    run("nested_definitions_restore_the_enclosing_scope");
}

/// Providers and saved JSON observe the supplied schema without authored provenance.
#[test]
fn provider_projection_does_not_emit_schema_metadata() {
    let authored = ParameterSchema::authored(
        serde_json::json!({
            "type": "object",
            "required": ["name"],
            "properties": {"name": {"type": "string"}},
        }),
        [
            (String::new(), SchemaKind::Object),
            ("/properties/name".to_owned(), SchemaKind::String),
        ],
    );
    let plain = ParameterSchema::from(serde_json::json!({
        "type": "object",
        "required": ["name"],
        "properties": {"name": {"type": "string"}},
    }));
    for schema in [authored, plain] {
        let tool = Tool {
            name: "echo".to_owned(),
            description: "Projected tool".to_owned(),
            parameters: schema,
        };
        let serialized = serde_json::to_value(&tool.parameters).expect("schema serialization");
        assert_eq!(serialized, tool.parameters.as_json().clone());
        assert_projections(&tool);
        let context = maestro_models::Context {
            system_prompt: None,
            messages: Vec::new(),
            tools: Some(vec![tool.clone()]),
        };
        let saved = serde_json::to_value(&context).expect("context serialization");
        assert_eq!(
            saved,
            serde_json::json!({
                "messages": [],
                "tools": [{
                    "name": "echo",
                    "description": "Projected tool",
                    "parameters": tool.parameters.as_json().clone(),
                }],
            }),
            "saved context carries the whole supplied schema"
        );
        let restored: maestro_models::Context =
            serde_json::from_value(saved).expect("context deserialization");
        let restored_tool = &restored.tools.expect("tools")[0];
        assert!(restored_tool.parameters.conversions().is_empty());
    }
}

/// Assert both content-provider projection modes carry the whole supplied schema.
#[track_caller]
fn assert_projections(tool: &Tool) {
    for (use_parameters, field) in [(true, "parameters"), (false, "parametersJsonSchema")] {
        let declarations = maestro_models::providers::content::google_shared::convert_tools(
            std::slice::from_ref(tool),
            use_parameters,
        );
        assert!(declarations.is_some(), "one declaration for {field}");
        let declared = serde_json::to_value(&declarations).unwrap_or_default();
        assert_eq!(
            declared,
            serde_json::json!([{
                "functionDeclarations": [{
                    "name": "echo",
                    "description": "Projected tool",
                    field: tool.parameters.as_json().clone(),
                }],
            }]),
            "{field} projection carries the whole supplied schema"
        );
    }
}

/// Members convert through their own nodes at every recorded corpus size.
#[test]
fn authored_schema_walk_has_no_property_rescan() {
    for size in [16, 32, 64] {
        assert_declared_members(size);
        assert_record_members(size);
        assert_members_are_not_rescanned(size);
    }
}

/// Assert a declared member converts once, never again through the additional schema.
#[track_caller]
fn assert_members_are_not_rescanned(size: usize) {
    let mut properties = Map::new();
    let mut arguments = Map::new();
    let mut expected = Map::new();
    let mut kinds = std::collections::HashMap::new();
    kinds.insert(String::new(), SchemaKind::Object);
    for index in 0..size {
        let name = format!("m{index}");
        properties.insert(name.clone(), serde_json::json!({"type": "string"}));
        arguments.insert(name.clone(), Value::Null);
        expected.insert(name, Value::from("null"));
        kinds.insert(format!("/properties/m{index}"), SchemaKind::String);
    }
    kinds.insert("/additionalProperties".to_owned(), SchemaKind::Null);
    let schema = ParameterSchema::authored(
        serde_json::json!({
            "type": "object",
            "required": arguments.keys().cloned().collect::<Vec<_>>(),
            "properties": properties,
            "additionalProperties": {"type": "null"},
        }),
        kinds,
    );
    assert_eq!(
        prepared(schema, Value::Object(arguments)),
        Value::Object(expected),
        "{size} declared members convert once"
    );
}

/// Assert every declared member of a wide object converts through its own node.
#[track_caller]
fn assert_declared_members(size: usize) {
    let mut properties = Map::new();
    let mut arguments = Map::new();
    let mut expected = Map::new();
    let mut kinds = std::collections::HashMap::new();
    kinds.insert(String::new(), SchemaKind::Object);
    for index in 0..size {
        let name = format!("m{index}");
        properties.insert(name.clone(), serde_json::json!({"type": "string"}));
        arguments.insert(name.clone(), Value::Null);
        if index % 2 == 0 {
            kinds.insert(format!("/properties/{name}"), SchemaKind::String);
            expected.insert(name, Value::from("null"));
        } else {
            expected.insert(name, Value::from(""));
        }
    }
    let schema = ParameterSchema::authored(
        serde_json::json!({
            "type": "object",
            "required": arguments.keys().cloned().collect::<Vec<_>>(),
            "properties": properties,
        }),
        kinds,
    );
    let prepared = prepared(schema, Value::Object(arguments));
    assert_eq!(prepared, Value::Object(expected), "{size} declared members");
}

/// Assert a wide record converts the members its pattern covers and leaves the others.
#[track_caller]
fn assert_record_members(size: usize) {
    let record = ParameterSchema::authored(
        serde_json::json!({
            "type": "object",
            "required": ["value"],
            "properties": {"value": {
                "type": "object",
                "patternProperties": {"^p": {"type": "string"}},
            }},
        }),
        [
            (String::new(), SchemaKind::Object),
            ("/properties/value".to_owned(), SchemaKind::Record),
            (
                "/properties/value/patternProperties/^p".to_owned(),
                SchemaKind::String,
            ),
        ],
    );
    let mut members = Map::new();
    let mut record_expected = Map::new();
    for index in 0..size {
        members.insert(format!("p{index}"), Value::Null);
        record_expected.insert(format!("p{index}"), Value::from("null"));
        members.insert(format!("extra{index}"), Value::from(1));
        record_expected.insert(format!("extra{index}"), Value::from(1));
    }
    let mut arguments = Map::new();
    arguments.insert("value".to_owned(), Value::Object(members));
    let mut expected = Map::new();
    expected.insert("value".to_owned(), Value::Object(record_expected));
    assert_eq!(
        prepared(record, Value::Object(arguments)),
        Value::Object(expected),
        "{size} record members"
    );
}

/// Prepare one tool call, failing the calling test with its diagnostic.
#[track_caller]
fn prepared(parameters: ParameterSchema, arguments: Value) -> Value {
    let outcome = prepare(parameters, arguments);
    assert!(outcome.is_ok(), "prepared arguments");
    outcome.unwrap_or_default()
}

/// Prepare one tool call through the public validator.
fn prepare(parameters: ParameterSchema, arguments: Value) -> Result<Value, String> {
    let tool = Tool {
        name: "echo".to_owned(),
        description: "Recorded case".to_owned(),
        parameters,
    };
    let call = ToolCall {
        id: "call".to_owned(),
        name: "echo".to_owned(),
        arguments,
        thought_signature: None,
    };
    validate_tool_arguments(&tool, &call).map_err(|error| error.message)
}

/// Declared kinds and their JSON pointer addresses.
fn declared(entries: &[(&str, SchemaKind)]) -> Vec<(String, SchemaKind)> {
    entries
        .iter()
        .map(|(pointer, kind)| ((*pointer).to_owned(), kind.clone()))
        .collect()
}

/// Each kind compares under its own rules and every member keeps its declared optionality.
///
/// An alternative set never becomes a sequence of the same positions or an intersection of
/// the same members, so its alternatives keep converting where the reference converts them.
/// A member an object leaves optional never satisfies a required member of another object.
#[test]
fn comparison_keeps_each_kind_and_modifier_separate() {
    run("comparison_keeps_each_kind_and_modifier_separate");
}

/// Assert the prepared arguments one authored schema yields.
#[track_caller]
fn assert_prepared(
    entries: &[(&str, SchemaKind)],
    schema: Value,
    arguments: impl Into<Value>,
    expected: impl Into<Value>,
) {
    let arguments: Value = arguments.into();
    let outcome = prepare(
        ParameterSchema::authored(schema, declared(entries)),
        arguments,
    );
    assert!(outcome.is_ok(), "prepared arguments: {:?}", outcome.err());
    assert_eq!(outcome.unwrap_or_default(), expected.into());
}

/// Assert the diagnostic one authored schema rejects its arguments with.
#[track_caller]
fn assert_rejected(
    entries: &[(&str, SchemaKind)],
    schema: Value,
    arguments: impl Into<Value>,
    expected: &str,
) {
    let arguments: Value = arguments.into();
    let outcome = prepare(
        ParameterSchema::authored(schema, declared(entries)),
        arguments,
    );
    assert_eq!(outcome.err().as_deref(), Some(expected));
}

/// Assert the prepared arguments one plain schema yields.
#[track_caller]
fn assert_plain_prepared(schema: Value, arguments: impl Into<Value>, expected: impl Into<Value>) {
    let arguments: Value = arguments.into();
    let prepared = prepared(ParameterSchema::from(schema), arguments);
    assert_eq!(prepared, expected.into());
}

/// An intersection keeps every operand's own member identity.
///
/// A member declared beside the converted one shares no kind with it, however their names
/// relate: the evaluated object keeps the exact keys and the child kinds its operands
/// declared, so a plain `a` stays plain beside a converted `ab`.
#[test]
fn intersection_composition_keeps_member_identity() {
    let number = serde_json::json!({
        "type": "object", "required": ["value"],
        "properties": {"value": {"allOf": [
            {"type": "object", "required": ["a", "ab"],
             "properties": {"a": {"type": "string"}, "ab": {"type": "number"}}},
            {"type": "object"},
        ]}},
    });
    assert_prepared(
        &[
            ("", SchemaKind::Object),
            ("/properties/value", SchemaKind::Intersect),
            ("/properties/value/allOf/0", SchemaKind::Object),
            (
                "/properties/value/allOf/0/properties/ab",
                SchemaKind::Number,
            ),
            ("/properties/value/allOf/1", SchemaKind::Object),
        ],
        number,
        serde_json::json!({"value": {"a": null, "ab": 1}}),
        serde_json::json!({"value": {"a": "", "ab": 1}}),
    );
    let text = serde_json::json!({
        "type": "object", "required": ["value"],
        "properties": {"value": {"allOf": [
            {"type": "object", "required": ["a", "ab"],
             "properties": {"a": {"type": "string"}, "ab": {"type": "string"}}},
            {"type": "object"},
        ]}},
    });
    assert_prepared(
        &[
            ("", SchemaKind::Object),
            ("/properties/value", SchemaKind::Intersect),
            ("/properties/value/allOf/0", SchemaKind::Object),
            (
                "/properties/value/allOf/0/properties/ab",
                SchemaKind::String,
            ),
            ("/properties/value/allOf/1", SchemaKind::Object),
        ],
        text,
        serde_json::json!({"value": {"a": null, "ab": 1}}),
        serde_json::json!({"value": {"a": "", "ab": "1"}}),
    );
}

/// An intersection's operands keep the definitions their cyclic node declared.
///
/// A reference operand that cannot recurse becomes the definition it names; a recursive one
/// keeps its definition until it is converted. Neither loses the kinds the definition carries.
#[test]
fn intersection_operands_keep_cyclic_definitions() {
    let scalar = serde_json::json!({
        "type": "object", "required": ["value"],
        "properties": {"value": {
            "$defs": {
                "S": {"type": "string", "$id": "S"},
                "R": {"allOf": [{"$ref": "S"}, {}], "$id": "R"},
            },
            "$ref": "R",
        }},
    });
    assert_prepared(
        &[
            ("", SchemaKind::Object),
            ("/properties/value", SchemaKind::Cyclic),
            ("/properties/value/$defs/S", SchemaKind::String),
            ("/properties/value/$defs/R", SchemaKind::Intersect),
            ("/properties/value/$defs/R/allOf/0", SchemaKind::Ref),
            ("/properties/value/$defs/R/allOf/1", SchemaKind::Unknown),
        ],
        scalar,
        serde_json::json!({"value": null}),
        serde_json::json!({"value": "null"}),
    );
    let object = serde_json::json!({
        "type": "object", "required": ["value"],
        "properties": {"value": {
            "$defs": {
                "S": {"type": "string", "$id": "S"},
                "R": {"allOf": [
                    {"type": "object", "required": ["a"], "properties": {"a": {"$ref": "S"}}},
                    {},
                ], "$id": "R"},
            },
            "$ref": "R",
        }},
    });
    assert_prepared(
        &[
            ("", SchemaKind::Object),
            ("/properties/value", SchemaKind::Cyclic),
            ("/properties/value/$defs/S", SchemaKind::String),
            ("/properties/value/$defs/R", SchemaKind::Intersect),
            ("/properties/value/$defs/R/allOf/0", SchemaKind::Object),
            (
                "/properties/value/$defs/R/allOf/0/properties/a",
                SchemaKind::Ref,
            ),
            ("/properties/value/$defs/R/allOf/1", SchemaKind::Object),
        ],
        object,
        serde_json::json!({"value": {"a": null}}),
        serde_json::json!({"value": {"a": "null"}}),
    );
}

/// A recursive definition converts every member it reaches again.
#[test]
fn recursive_definitions_convert_nested_members() {
    let schema = serde_json::json!({
        "type": "object", "required": ["value"],
        "properties": {"value": {
            "$defs": {
                "S": {"type": "object", "required": ["a"],
                      "properties": {"a": {"type": "string"}, "next": {"$ref": "S"}},
                      "$id": "S"},
                "R": {"allOf": [{"$ref": "S"}, {}], "$id": "R"},
            },
            "$ref": "R",
        }},
    });
    assert_prepared(
        &[
            ("", SchemaKind::Object),
            ("/properties/value", SchemaKind::Cyclic),
            ("/properties/value/$defs/S", SchemaKind::Object),
            ("/properties/value/$defs/S/properties/a", SchemaKind::String),
            ("/properties/value/$defs/S/properties/next", SchemaKind::Ref),
            ("/properties/value/$defs/R", SchemaKind::Intersect),
            ("/properties/value/$defs/R/allOf/0", SchemaKind::Ref),
            ("/properties/value/$defs/R/allOf/1", SchemaKind::Unknown),
        ],
        schema,
        serde_json::json!({"value": {"a": null, "next": {"a": null}}}),
        serde_json::json!({"value": {"a": "null", "next": {"a": "null"}}}),
    );
}

/// A plain reference keeps the last matching subtree below its resource root.
///
/// The root's own definitions win; without one, the last sibling subtree that declares the
/// pointer answers, so a later declaration is never shadowed by an earlier one.
#[test]
fn plain_reference_selection_uses_the_last_subtree() {
    let nested = serde_json::json!({
        "type": "object",
        "required": ["value"],
        "properties": {"value": {"$ref": "#/$defs/S"}},
        "left": {"$defs": {"S": {"type": "number", "$id": "S"}}},
        "right": {"$defs": {"S": {"type": "string", "$id": "S"}}},
    });
    let diagnostic = "Validation failed for tool \"echo\":\n  - value: must be string\n\nReceived arguments:\n{\n  \"value\": 1\n}";
    assert_rejected(&[], nested, serde_json::json!({"value": 1}), diagnostic);
    let rooted = serde_json::json!({
        "type": "object",
        "required": ["value"],
        "properties": {"value": {"$ref": "#/$defs/S"}},
        "$defs": {"S": {"type": "number", "$id": "S"}},
        "left": {"$defs": {"S": {"type": "string", "$id": "S"}}},
    });
    assert_plain_prepared(
        rooted,
        serde_json::json!({"value": 1}),
        serde_json::json!({"value": 1}),
    );
}

/// A sequence root returns a converted sequence while its own members stay as supplied.
///
/// The reference converter maps a sequence into a new one and keeps nested container edits in
/// the sequence it was given, so the later plain pass sees the members it had.
#[test]
fn sequence_roots_keep_returned_and_retained_effects_apart() {
    let text = serde_json::json!({
        "type": "object", "required": ["value"],
        "properties": {"value": {"type": "string"}},
    });
    assert_plain_prepared(
        text,
        serde_json::json!({"value": null}),
        serde_json::json!({"value": ""}),
    );
    let array = serde_json::json!({
        "type": "object", "required": ["value"],
        "properties": {"value": {"type": "array", "items": {"type": "string"}}},
    });
    assert_plain_prepared(
        array,
        serde_json::json!({"value": [null]}),
        serde_json::json!({"value": [""]}),
    );
}

/// A converted nested tuple edits the containers the supplied value already holds.
#[test]
fn nested_container_edits_survive_a_returned_sequence() {
    let tuple = serde_json::json!({
        "type": "object", "required": ["value"],
        "properties": {"value": {
            "type": "array",
            "items": {"type": "array", "items": [{"type": "string"}]},
        }},
    });
    assert_prepared(
        &[
            ("", SchemaKind::Object),
            ("/properties/value", SchemaKind::Array),
            ("/properties/value/items", SchemaKind::Tuple),
            ("/properties/value/items/items/0", SchemaKind::String),
        ],
        tuple,
        serde_json::json!({"value": [[null]]}),
        serde_json::json!({"value": [["null"]]}),
    );
    let object = serde_json::json!({
        "type": "object", "required": ["value"],
        "properties": {"value": {
            "type": "array",
            "items": {"type": "array", "items": {
                "type": "object", "required": ["value"],
                "properties": {"value": {"type": "string"}},
            }},
        }},
    });
    assert_prepared(
        &[
            ("", SchemaKind::Object),
            ("/properties/value", SchemaKind::Array),
            ("/properties/value/items", SchemaKind::Array),
            ("/properties/value/items/items", SchemaKind::Object),
            (
                "/properties/value/items/items/properties/value",
                SchemaKind::String,
            ),
        ],
        object,
        serde_json::json!({"value": [[{"value": null}]]}),
        serde_json::json!({"value": [[{"value": "null"}]]}),
    );
}

/// Assert every recorded scope case of one group, failing when the group is missing.
#[track_caller]
fn run_scope(group: &str, count: usize) {
    let parsed: serde_json::Result<Vec<Case>> =
        serde_json::from_str(include_str!("fixtures/tool_argument_scope.json"));
    assert!(parsed.is_ok(), "recorded scope corpus");
    let cases: Vec<Case> = parsed
        .unwrap_or_default()
        .into_iter()
        .filter(|case| case.group == group)
        .collect();
    assert_eq!(cases.len(), count, "recorded {group} cases");
    let failed: Vec<&str> = cases
        .iter()
        .filter(|case| std::panic::catch_unwind(|| assert_case(case)).is_err())
        .map(|case| case.label.as_str())
        .collect();
    assert!(failed.is_empty(), "{group} cases differ: {failed:?}");
}

/// A union alternative is checked through the definitions its conversion sees, so a
/// reference alternative selects the candidate its definition accepts.
#[test]
fn union_candidates_check_through_the_converting_definitions() {
    run_scope("candidate checking", 2);
}

/// A definition block entered by one member stays out of its sibling's scope, and a
/// reference reads the definitions of the conversion that reaches it.
#[test]
fn sibling_definitions_keep_the_enclosing_scope() {
    run_scope("definition scope", 3);
}

/// A composed candidate keeps its members' optionality and its declared constraints.
#[test]
fn composed_candidates_keep_declared_constraints() {
    run_scope("composed candidates", 6);
}

#[path = "../../maestro-extensions-wasm/src/tests/nested_schema_fixture.rs"]
mod nested_schema_fixture;

/// Nested declarations keep conversion provenance without changing the invocation.
#[test]
fn nested_declarations_keep_authored_and_plain_preparation() {
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Invocation {
        /// Declaration identity.
        id: String,
        /// Supplied invocation text.
        arguments_json: String,
        /// Complete prepared result.
        expected_json: String,
    }
    let cases: Vec<Invocation> = serde_json::from_str(include_str!(
        "../../maestro-extensions-wasm/src/tests/nested_schema_invocations.json"
    ))
    .unwrap();
    for case in cases {
        let tool = Tool {
            name: "nested".into(),
            description: String::new(),
            parameters: nested_schema_fixture::parameters(&case.id).unwrap(),
        };
        let args: Value = serde_json::from_str(&case.arguments_json).unwrap();
        let call = ToolCall {
            id: "call".into(),
            name: "nested".into(),
            arguments: args.clone(),
            thought_signature: None,
        };
        let expected: Value = serde_json::from_str(&case.expected_json).unwrap();
        assert_eq!(
            validate_tool_arguments(&tool, &call),
            Ok(expected),
            "{}",
            case.id
        );
        assert_eq!(call.arguments, args, "{} invocation unchanged", case.id);
    }
}
