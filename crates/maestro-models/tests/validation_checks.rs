use maestro_models::*;
use serde_json::{Value, json};
use std::collections::BTreeMap;
fn call(arguments: Value) -> ToolCall {
    ToolCall {
        id: "id".into(),
        name: "echo".into(),
        arguments,
        thought_signature: None,
    }
}
fn tool(parameters: impl Into<TSchema>) -> Tool {
    Tool {
        name: "echo".into(),
        description: "Echo tool".into(),
        parameters: parameters.into(),
    }
}
fn metadata(schema: Value, kinds: &[(&[&str], &str)], legacy: bool) -> TSchema {
    TSchema::new(
        schema,
        kinds
            .iter()
            .map(|(path, kind)| (path.iter().map(|s| (*s).into()).collect(), (*kind).into()))
            .collect(),
        legacy,
    )
}
fn validate(schema: impl Into<TSchema>, arguments: Value) -> Result<Value, ThrownValue> {
    validate_tool_arguments(&tool(schema), &call(arguments))
}
fn text(error: ThrownValue) -> String {
    format_thrown_value(&error).unwrap()
}

#[test]
fn metadata_number_validates_without_dynamic_codegen() {
    let schema = metadata(
        json!({"type":"object","properties":{"count":{"type":"number"}},"required":["count"]}),
        &[(&[], "Object"), (&["properties", "count"], "Number")],
        false,
    );
    assert_eq!(
        validate(schema, json!({"count":"42"})).unwrap(),
        json!({"count":42})
    );
}

#[test]
fn plain_schema_primitive_table_matches_expected_values() {
    for (kind, input, expected) in [
        (json!("number"), json!("42"), json!(42)),
        (json!("number"), json!(true), json!(1)),
        (json!("number"), json!(null), json!(0)),
        (json!("integer"), json!("42"), json!(42)),
        (json!("boolean"), json!("true"), json!(true)),
        (json!("boolean"), json!("false"), json!(false)),
        (json!("boolean"), json!(1), json!(true)),
        (json!("boolean"), json!(0), json!(false)),
        (json!("string"), json!(null), json!("")),
        (json!("string"), json!(true), json!("true")),
        (json!("null"), json!(""), json!(null)),
        (json!("null"), json!(0), json!(null)),
        (json!("null"), json!(false), json!(null)),
        (json!(["number", "string"]), json!("1"), json!("1")),
        (json!(["boolean", "number"]), json!("1"), json!(1)),
    ] {
        assert_eq!(
            validate(
                json!({"type":"object","properties":{"value":{"type":kind}},"required":["value"]}),
                json!({"value":input})
            )
            .unwrap(),
            json!({"value":expected})
        );
    }
}

#[test]
fn plain_schema_invalid_coercions_report_validation() {
    for (kind, input) in [
        ("boolean", "1"),
        ("boolean", "0"),
        ("null", "null"),
        ("integer", "42.1"),
    ] {
        let declaration = tool(
            json!({"type":"object","properties":{"value":{"type":kind}},"required":["value"]}),
        );
        let call = call(json!({"value":input}));
        let before = call.clone();
        assert_eq!(
            text(validate_tool_arguments(&declaration, &call).unwrap_err()),
            format!(
                "Validation failed for tool \"echo\":\n  - value: must be {kind}\n\nReceived arguments:\n{{\n  \"value\": \"{input}\"\n}}"
            )
        );
        assert_eq!(call, before);
    }
}

#[test]
fn tool_lookup_uses_first_exact_name() {
    let mut call = call(json!("bad"));
    call.name = "Echo".into();
    assert_eq!(
        text(validate_tool_call(&[], &call).unwrap_err()),
        "Tool \"Echo\" not found"
    );
    assert_eq!(
        text(validate_tool_call(&[tool(json!({}))], &call).unwrap_err()),
        "Tool \"Echo\" not found"
    );
    call.name = "echo".into();
    assert_eq!(
        validate_tool_call(&[tool(json!({})), tool(json!({"type":"number"}))], &call).unwrap(),
        json!("bad")
    );
    assert!(validate_tool_call(&[tool(json!({"type":"number"})), tool(json!({}))], &call).is_err());
    call.name = "different".into();
    assert_eq!(
        validate_tool_arguments(&tool(json!({})), &call).unwrap(),
        json!("bad")
    );
    assert_eq!(
        text(validate_tool_arguments(&tool(json!({"type":"number"})), &call).unwrap_err()),
        "Validation failed for tool \"different\":\n  - root: must be number\n\nReceived arguments:\n\"bad\""
    );
}

#[test]
fn plain_primitive_edges_keep_number_and_boolean_rules() {
    for (kind, input, expected) in [
        (json!("number"), json!(".5"), json!(0.5)),
        (json!("number"), json!("+1."), json!(1)),
        (json!("number"), json!("1e2"), json!(100)),
        (json!("integer"), json!("0x10"), json!(16)),
        (json!("number"), json!("0o10"), json!(8)),
        (json!("number"), json!("0b10"), json!(2)),
        (json!("number"), json!(false), json!(0)),
        (json!("boolean"), json!(null), json!(false)),
        (json!("string"), json!(false), json!("false")),
        (json!("string"), json!(2.5), json!("2.5")),
        (json!(["number", "string"]), json!("42"), json!("42")),
        (json!(["boolean", "number"]), json!("1"), json!(1)),
        (json!(["integer", "number"]), json!(1.5), json!(1.5)),
    ] {
        assert_eq!(
            validate(
                json!({"type":"object","properties":{"v":{"type":kind}}}),
                json!({"v":input})
            )
            .unwrap(),
            json!({"v":expected})
        );
    }
    for (kind, input) in [
        ("integer", json!("42.1")),
        ("number", json!("")),
        ("number", json!(" \n")),
        ("number", json!("Infinity")),
        ("number", json!("NaN")),
        ("number", json!("1e400")),
        ("number", json!("+0x10")),
        ("number", json!("-0x10")),
        ("number", json!("0x")),
        ("number", json!("0o8")),
        ("number", json!("0b2")),
        ("boolean", json!("TRUE")),
        ("boolean", json!("1")),
        ("boolean", json!(2)),
        ("null", json!("null")),
        ("null", json!(true)),
        ("null", json!(1)),
        ("string", json!([])),
        ("number", json!({})),
    ] {
        assert!(
            validate(
                json!({"type":"object","properties":{"v":{"type":kind}}}),
                json!({"v":input})
            )
            .is_err(),
            "{kind} {input}"
        );
    }
    for input in [json!({}), json!([]), json!(null), json!(true)] {
        assert_eq!(validate(json!({}), input.clone()).unwrap(), input);
    }
    assert_eq!(
        validate(
            json!({"type":"object","properties":{"v":{"type":[42,"number"]}}}),
            json!({"v":"2"})
        )
        .unwrap(),
        json!({"v":2})
    );
}

#[test]
fn metadata_and_serialized_schemas_convert_differently() {
    for (kind, type_, input, expected) in [
        ("Number", "number", json!(""), json!(0)),
        ("Number", "number", json!("TRUE"), json!(1)),
        ("Integer", "integer", json!("42.9"), json!(42)),
        ("Boolean", "boolean", json!("TRUE"), json!(true)),
        ("Boolean", "boolean", json!("1"), json!(true)),
        ("String", "string", json!(null), json!("null")),
        ("Null", "null", json!("null"), json!(null)),
        ("Array", "array", json!("2"), json!([2])),
    ] {
        let child = if kind == "Array" {
            json!({"type":type_,"items":{"type":"number"}})
        } else {
            json!({"type":type_})
        };
        let schema = metadata(
            json!({"type":"object","properties":{"v":child}}),
            &[
                (&[], "Object"),
                (&["properties", "v"], kind),
                (&["properties", "v", "items"], "Number"),
            ],
            false,
        );
        assert_eq!(
            validate(schema.clone(), json!({"v":input})).unwrap(),
            json!({"v":expected}),
            "{kind}"
        );
        let reparsed: TSchema =
            serde_json::from_str(&serde_json::to_string(&schema).unwrap()).unwrap();
        if kind == "String" {
            assert_eq!(
                validate(reparsed, json!({"v":input})).unwrap(),
                json!({"v":""})
            );
        } else {
            assert!(validate(reparsed, json!({"v":input})).is_err(), "{kind}");
        }
    }
    let schema = metadata(
        json!({"type":"object","properties":{"v":{"type":"number"}}}),
        &[],
        true,
    );
    assert!(validate(schema, json!({"v":"42"})).is_err());
    let enumerable:TSchema=json!({"~kind":"Object","type":"object","properties":{"v":{"~kind":"Number","type":"number"}}}).into();
    assert_eq!(
        validate(enumerable, json!({"v":""})).unwrap(),
        json!({"v":0})
    );
}

#[test]
fn metadata_nested_conversion_keeps_container_rules() {
    let rows: Value =
        serde_json::from_str(include_str!("fixtures/schema_conversion.json")).unwrap();
    for row in rows.as_array().unwrap() {
        let kinds = row["kinds"]
            .as_array()
            .unwrap()
            .iter()
            .map(|pair| {
                (
                    pair[0]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|s| s.as_str().unwrap().to_string())
                        .collect(),
                    pair[1].as_str().unwrap().to_string(),
                )
            })
            .collect();
        let result = validate(
            TSchema::new(row["schema"].clone(), kinds, false),
            row["input"].clone(),
        );
        if let Some(expected) = row.get("expected") {
            assert_eq!(result.unwrap(), *expected, "{}", row["name"]);
        } else {
            assert_eq!(text(result.unwrap_err()), row["error"], "{}", row["name"]);
        }
    }
    let schema = metadata(
        json!({"type":"object","properties":{"v":{"anyOf":[{"type":"number"},{"type":"string"}]}}}),
        &[
            (&[], "Object"),
            (&["properties", "v"], "Union"),
            (&["properties", "v", "anyOf", "0"], "Number"),
            (&["properties", "v", "anyOf", "1"], "String"),
        ],
        true,
    );
    assert_eq!(
        validate(schema, json!({"v":"42"})).unwrap(),
        json!({"v":"42"})
    );
    let schema = metadata(json!({"type":"number"}), &[(&[], "Number")], true);
    assert!(validate(schema, json!("42")).is_err());
}

#[test]
fn object_properties_precede_additional_schema_values() {
    let schema = json!({"type":"object","properties":{"z":{"type":"integer"},"2":{"type":"boolean"},"a":{"type":"string"},"missing":{"type":"integer","default":7}},"additionalProperties":{"type":"number"}});
    assert_eq!(
        validate(schema, json!({"extra":"2.5","a":true,"z":"4","2":"true"})).unwrap(),
        json!({"extra":2.5,"a":"true","z":4,"2":true})
    );
    for additional in [true, false] {
        let result = validate(
            json!({"type":"object","properties":{"x":{"type":"number"}},"additionalProperties":additional}),
            json!({"x":"2","extra":"3"}),
        );
        if additional {
            assert_eq!(result.unwrap(), json!({"x":2,"extra":"3"}));
        } else {
            assert!(result.is_err());
        }
    }
    assert_eq!(
        validate(
            json!({"type":"object","properties":{"missing":{"default":1,"type":"number"}}}),
            json!({})
        )
        .unwrap(),
        json!({})
    );
}

#[test]
fn array_items_keep_tuple_and_homogeneous_behavior() {
    assert_eq!(
        validate(
            json!({"type":"array","items":{"type":"number"}}),
            json!(["2.5", null, true])
        )
        .unwrap(),
        json!([2.5, 0, 1])
    );
    assert_eq!(
        validate(
            json!({"type":"array","items":[{"type":"integer"},true,{"type":"boolean"}]}),
            json!(["2", "same", "true", "excess"])
        )
        .unwrap(),
        json!([2, "same", true, "excess"])
    );
    assert_eq!(
        validate(json!({"items":{"type":"integer"}}), json!("2")).unwrap(),
        json!("2")
    );
    assert_eq!(validate(json!({"type":"array","items":{"type":"object","properties":{"v":{"type":"array","items":{"type":"boolean"}}}}}),json!([{"v":[null,"false"]}])).unwrap(),json!([{"v":[false,false]}]));
}

#[test]
fn combinators_try_clones_in_declared_order() {
    let schema = json!({"type":"object","properties":{"v":{"allOf":[{"type":"number"},{"type":"string"}],"anyOf":[{"type":"integer","minimum":50},{"type":"boolean"}],"oneOf":[{"type":"boolean"},{"type":"null"}]}}});
    assert!(validate(schema, json!({"v":true})).is_err());
    let schema = json!({"type":"object","properties":{"v":{"allOf":[{"type":"integer"},{"minimum":0}],"anyOf":[{"type":"integer"},{"type":"boolean"}],"oneOf":[{"type":"integer"},{"type":"boolean"}]}}});
    assert_eq!(validate(schema, json!({"v":true})).unwrap(), json!({"v":1}));
    let schema = json!({"type":"object","properties":{"v":{"anyOf":[{"type":"object","properties":{"x":{"type":"integer"}},"required":["missing"]},{"type":"object","properties":{"x":{"type":"boolean"}}}]}}});
    assert_eq!(
        validate(schema, json!({"v":{"x":null}})).unwrap(),
        json!({"v":{"x":false}})
    );
    assert!(validate(json!({"type":"object","properties":{"v":{"oneOf":[{"type":"number"},{"type":"integer"}]}}}),json!({"v":"2"})).is_err());
    assert!(validate(json!({"type":"object","properties":{"v":{"anyOf":[{"type":"number"},{"type":"boolean"}]}}}),json!({"v":"bad"})).is_err());
}

#[test]
fn standalone_union_compile_failures_are_skipped() {
    let schema = json!({"$defs":{"limit":{"minimum":50}},"type":"object","properties":{"v":{"anyOf":[{"type":"integer","$ref":"#/$defs/limit"},{"type":"boolean"}]}}});
    assert_eq!(
        validate(schema, json!({"v":null})).unwrap(),
        json!({"v":false})
    );
    let schema = json!({"type":"object","properties":{"v":{"anyOf":[{"type":"number","minimum":5},{"type":"string","pattern":"["},{"type":"boolean"}]}}});
    assert!(validate(schema, json!({"v":null})).is_err());
    let schema = json!({"type":"object","properties":{"v":{"anyOf":[{"$ref":"#/missing"},{"type":"boolean"}]}}});
    assert_eq!(
        validate(schema, json!({"v":"true"})).unwrap(),
        json!({"v":true})
    );
}

#[test]
fn schema_shape_guards_accept_record_like_inputs() {
    for schema in [
        json!([]),
        json!({"type":"invalid"}),
        json!({"type":42}),
        json!({"required":true}),
        json!({"type":[42,"number"]}),
    ] {
        assert_eq!(validate(schema, json!("bad")).unwrap(), json!("bad"));
    }
    assert_eq!(
        validate(
            json!({"type":"object","properties":{"v":[]},"additionalProperties":[]}),
            json!({"v":"2","x":"3"})
        )
        .unwrap(),
        json!({"v":"2","x":"3"})
    );
}

#[test]
fn root_coercion_returns_original_when_candidate_fails() {
    assert_eq!(
        validate(json!({"type":"integer","minimum":50}), json!("42")).unwrap(),
        json!("42")
    );
    assert_eq!(
        validate(json!({"type":"integer"}), json!("42")).unwrap(),
        json!(42)
    );
    assert!(validate(json!({"type":"integer"}), json!("bad")).is_err());
    assert!(validate(json!({"allOf":[{"type":"integer"}]}), json!({"v":"42"})).is_err());
    assert_eq!(
        validate(
            json!({"anyOf":[{"type":"object","properties":{"v":{"type":"integer"}}}]}),
            json!({"v":"42"})
        )
        .unwrap(),
        json!({"v":42})
    );
}

#[test]
fn validation_success_and_failure_leave_inputs_unchanged() {
    let declaration = tool(
        json!({"type":"object","properties":{"v":{"type":"array","items":{"type":"object","properties":{"n":{"type":"integer"}}}}}}),
    );
    let schema = declaration.parameters.json();
    for input in [json!({"v":[{"n":"42"}]}), json!({"v":[{"n":"bad"}]})] {
        let call = call(input.clone());
        let result = validate_tool_arguments(&declaration, &call);
        if let Ok(mut result) = result {
            result["v"][0]["n"] = json!(9);
        }
        assert_eq!(call.arguments, input);
        assert_eq!(declaration.parameters.json(), schema);
    }
}

#[test]
fn schema_identity_reuses_compilation_without_invalidation() {
    let schema: TSchema = json!({"type":"object","properties":{"v":{"type":"number"}}}).into();
    let shared = schema.clone();
    assert_eq!(schema, shared);
    assert_ne!(schema, TSchema::from(schema.json()));
    assert_eq!(
        validate(schema.clone(), json!({"v":2})).unwrap(),
        json!({"v":2})
    );
    schema.replace(
        json!({"type":"object","properties":{"v":{"type":"string"}}}),
        BTreeMap::new(),
        false,
    );
    assert_eq!(
        text(validate(shared, json!({"v":"bad"})).unwrap_err()),
        "Validation failed for tool \"echo\":\nUnknown validation error\n\nReceived arguments:\n{\n  \"v\": \"bad\"\n}"
    );
    assert_eq!(
        validate(TSchema::from(schema.json()), json!({"v":"bad"})).unwrap(),
        json!({"v":"bad"})
    );
    let failed: TSchema = json!({"pattern":"["}).into();
    assert!(validate(failed.clone(), json!("x")).is_err());
    failed.replace(json!({"type":"string"}), BTreeMap::new(), false);
    assert_eq!(validate(failed, json!("x")).unwrap(), json!("x"));
}

#[test]
fn validation_messages_keep_paths_order_and_received_json() {
    for (schema, input, errors) in [
        (
            json!({"type":"object","required":["a","b"]}),
            json!({}),
            "  - a: must have required properties a, b",
        ),
        (
            json!({"type":"object","properties":{"x":{"type":"object","required":["a"]}}}),
            json!({"x":{}}),
            "  - x.a: must have required properties a",
        ),
        (
            json!({"type":"integer"}),
            json!("bad"),
            "  - root: must be integer",
        ),
        (
            json!({"type":"object","properties":{"a/b~x":{"type":"integer"}}}),
            json!({"a/b~x":"bad"}),
            "  - a.b~x: must be integer",
        ),
        (
            json!({"type":"object","properties":{"z":{"type":"number"},"a":{"type":"boolean"}},"required":["missing","other"]}),
            json!({"z":"bad","a":"no"}),
            "  - missing: must have required properties missing, other\n  - z: must be number\n  - a: must be boolean",
        ),
    ] {
        assert_eq!(
            text(validate(schema, input.clone()).unwrap_err()),
            format!(
                "Validation failed for tool \"echo\":\n{errors}\n\nReceived arguments:\n{}",
                serde_json::to_string_pretty(&input).unwrap()
            )
        );
    }
}

#[test]
fn json_diagnostics_keep_index_order_and_number_tokens() {
    let input: Value = serde_json::from_str(
        r#"{"10":10,"2":2,"01":1,"0":0,"4294967294":4,"4294967295":5,"z":-0.0}"#,
    )
    .unwrap();
    let expected = "{\n  \"0\": 0,\n  \"2\": 2,\n  \"10\": 10,\n  \"4294967294\": 4,\n  \"01\": 1,\n  \"4294967295\": 5,\n  \"z\": 0\n}";
    assert_eq!(
        text(validate(json!({"not":{}}), input).unwrap_err()),
        format!(
            "Validation failed for tool \"echo\":\n  - root: must not be valid\n\nReceived arguments:\n{expected}"
        )
    );
    for (input, expected) in [
        (json!(-0.0), "0"),
        (json!(1e-6), "0.000001"),
        (json!(1e20), "100000000000000000000"),
        (json!(1e21), "1e+21"),
        (json!(f64::from_bits(1)), "5e-324"),
        (json!(9007199254740993u64), "9007199254740992"),
        (json!("\0\t\n/😀"), r#""\u0000\t\n/😀""#),
    ] {
        assert_eq!(
            text(validate(json!({"not":{}}), input).unwrap_err()),
            format!(
                "Validation failed for tool \"echo\":\n  - root: must not be valid\n\nReceived arguments:\n{expected}"
            )
        );
    }
}

#[test]
fn number_strings_keep_ecmascript_round_trip_spelling() {
    for (value, expected) in [
        (json!(0), "0"),
        (json!(-0.0), "0"),
        (json!(42), "42"),
        (json!(-42), "-42"),
        (json!(2.5), "2.5"),
        (json!(-2.5), "-2.5"),
        (json!(1e-7), "1e-7"),
        (json!(-1e-7), "-1e-7"),
        (json!(1e-6), "0.000001"),
        (json!(-1e-6), "-0.000001"),
        (json!(1e20), "100000000000000000000"),
        (json!(-1e20), "-100000000000000000000"),
        (json!(1e21), "1e+21"),
        (json!(-1e21), "-1e+21"),
        (json!(1e23), "1e+23"),
        (json!(f64::from_bits(1)), "5e-324"),
        (json!(f64::MAX), "1.7976931348623157e+308"),
        (json!(1.2345678901234567), "1.2345678901234567"),
        (json!(9007199254740993u64), "9007199254740992"),
        (json!(1000000000000000128u64), "1000000000000000100"),
    ] {
        assert_eq!(
            validate(
                json!({"type":"object","properties":{"v":{"type":"string","enum":[expected]}}}),
                json!({"v":value})
            )
            .unwrap(),
            json!({"v":expected})
        );
    }
}

#[test]
fn offline_checker_keeps_corrective_information() {
    let rows: Value = serde_json::from_str(include_str!("fixtures/schema_errors.json")).unwrap();
    for row in rows.as_array().unwrap() {
        let root = if row["schema"].is_boolean() {
            json!({"allOf":[row["schema"].clone()]})
        } else {
            row["schema"].clone()
        };
        let schema = TSchema::new(root, BTreeMap::new(), true);
        let result = validate(schema, row["input"].clone());
        if row["valid"] == true {
            assert_eq!(result.unwrap(), row["input"], "{}", row["schema"]);
        } else {
            let lines = row["errors"]
                .as_array()
                .unwrap()
                .iter()
                .map(|error| {
                    let mut path = error["instancePath"]
                        .as_str()
                        .unwrap()
                        .trim_start_matches('/')
                        .replace('/', ".");
                    if error["keyword"] == "required"
                        && let Some(key) = error["params"]["requiredProperties"][0]
                            .as_str()
                            .filter(|s| !s.is_empty())
                    {
                        if !path.is_empty() {
                            path.push('.');
                        }
                        path.push_str(key);
                    }
                    if path.is_empty() {
                        path = "root".into();
                    }
                    format!("  - {path}: {}", error["message"].as_str().unwrap())
                })
                .collect::<Vec<_>>()
                .join("\n");
            let expected = format!(
                "Validation failed for tool \"echo\":\n{lines}\n\nReceived arguments:\n{}",
                serde_json::to_string_pretty(&row["input"]).unwrap()
            );
            let error = match result {
                Err(e) => e,
                Ok(v) => panic!("schema {} accepted {v}", row["schema"]),
            };
            assert_eq!(text(error), expected, "{}", row["schema"]);
        }
    }
    for reference in ["https://example.invalid/schema", "file:///unavailable.json"] {
        assert!(validate(json!({"$ref":reference}), json!({})).is_err());
    }
}

#[test]
fn schema_metadata_serialization_omits_runtime_markers() {
    let json = json!({"type":"object","properties":{"v":{"type":"number"}}});
    let schema = metadata(
        json.clone(),
        &[(&[], "Object"), (&["properties", "v"], "Number")],
        true,
    );
    let shared = schema.clone();
    assert_eq!(schema, shared);
    assert_eq!(schema.json(), json);
    assert_eq!(serde_json::to_value(&schema).unwrap(), json);
    let declaration = tool(schema.clone());
    assert_eq!(
        serde_json::to_value(declaration).unwrap()["parameters"],
        json
    );
    let plain: TSchema = serde_json::from_value(json.clone()).unwrap();
    assert_ne!(schema, plain);
    assert!(validate(schema.clone(), json!({"v":"bad"})).is_err());
    let mut snapshot = schema.json();
    snapshot["properties"]["v"]["type"] = json!("string");
    assert_eq!(schema.json(), json);
    shared.replace(
        json!({"type":"object","properties":{"v":{"type":"string"}}}),
        BTreeMap::new(),
        false,
    );
    assert_eq!(schema.json(), shared.json());
    assert_eq!(schema, shared);
    assert!(!format!("{schema:?}").contains("properties"));
}

#[test]
fn argument_guide_matches_the_exposed_contract() {
    let guide = include_str!("../../../docs/models/arguments.md");
    assert!(guide.contains(r#"`parse_streaming_json` provides a best-effort value for display. It tries complete parsing with string repair, partial parsing, and partial parsing after string repair, in that order. If none succeeds it returns an empty object. Parsing neither validates arguments nor executes a tool."#));
    assert!(guide.contains(r#"`parse_json_with_repair` first tries the supplied text. It retries only when string repair changes that text, and reports the error from the attempt that failed. Parsed roots may be objects, arrays or primitives. Lone UTF-16 surrogate escapes become U+FFFD; valid pairs become their character. Out-of-range numbers are unreadable. The separate `sanitize_surrogates` text helper removes unpaired UTF-16 units."#));
    assert!(guide.contains(r#"`validate_tool_call` selects the first exact tool name; `validate_tool_arguments` validates against an explicitly supplied declaration. Both clone arguments before conversion. Metadata-bearing schemas and plain serialized schemas have distinct conversion behavior. A shared `TSchema` keeps its identity and compiled checker across replacement, while JSON serialization omits hidden schema metadata."#));
    assert!(guide.contains(r#"Validation errors include the call's tool name, corrective property or root paths, and the original received arguments as two-space-indented JSON. Neither successful nor failed validation mutates the supplied call. When primitive-root conversion changes the value but the converted value fails its schema, validation returns the original root without an error; callers must not infer authorization from that result."#));
    assert!(guide.contains(r#"Schema checking is offline: it retrieves no HTTP or file references. Union alternatives are checked independently, and an unbuildable alternative is skipped. Reader and checker diagnostics use the established wording, order and locations. Partial display values are never an instruction to execute a tool."#));
    assert_eq!(
        include_str!("../third-party/partial-json-LICENSE"),
        r#"MIT License

Copyright (c) 2023 Promplate Dev Team

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
"#
    );
    use maestro_models::{Tool, ToolCall, parse_streaming_json, validate_tool_call};
    use serde_json::json;

    let partial = parse_streaming_json(Some(r#"{"count":"4"#));
    assert_eq!(partial, json!({"count":"4"}));
    let tools = [Tool {
        name: "echo".into(),
        description: "Echo tool".into(),
        parameters: json!({
            "type":"object", "properties":{"count":{"type":"number"}},
            "required":["count"]
        })
        .into(),
    }];
    let call = ToolCall {
        id: "tool-1".into(),
        name: "echo".into(),
        arguments: json!({"count":"42"}),
        thought_signature: None,
    };
    let converted = validate_tool_call(&tools, &call).unwrap();
    assert_eq!(converted, json!({"count":42}));
    assert_eq!(call.arguments, json!({"count":"42"}));
}

#[test]
fn metadata_patterns_keep_utf16_matching_and_literal_replacement() {
    for (pattern, key) in [
        (r"^\s$", "\u{feff}"),
        (r"^..$", "😀"),
        (r"(?<=a)b", "ab"),
        (r"^(a)\1$", "aa"),
        (r"^(a)(b)$", "ab"),
        (r"^\$$", "$"),
    ] {
        let schema = metadata(
            json!({"type":"object","properties":{"v":{"type":"object","patternProperties":{pattern:{"type":"number"}}}}}),
            &[
                (&[], "Object"),
                (&["properties", "v"], "Record"),
                (&["properties", "v", "patternProperties", pattern], "Number"),
            ],
            true,
        );
        assert_eq!(
            validate(schema, json!({"v":{key:""}})).unwrap(),
            json!({"v":{key:0}}),
            "{pattern} {key}"
        );
    }
    for (pattern, key) in [(r"^\s$", "\u{85}"), (r"^.$", "😀")] {
        let schema = metadata(
            json!({"type":"object","patternProperties":{pattern:{"type":"number"}}}),
            &[(&[], "Record"), (&["patternProperties", pattern], "Number")],
            true,
        );
        let result = validate(schema, json!({key:""}));
        if key == "😀" {
            assert_eq!(
                text(result.unwrap_err()),
                "Validation failed for tool \"echo\":\n  - 😀: must be number\n\nReceived arguments:\n{\n  \"😀\": \"\"\n}"
            );
        } else {
            assert_eq!(result.unwrap(), json!({key:""}));
        }
    }
    assert_eq!(sanitize_surrogates(&[0xd800, 36, 49, 0xdc00]), "$1");
}

#[test]
fn primitive_schema_roots_fail_with_the_uncacheable_key_error() {
    for schema in [json!(false), json!(null), json!(42), json!("schema")] {
        let schema: TSchema = schema.into();
        for _ in 0..2 {
            let ThrownValue::Error(error) = validate(schema.clone(), json!({})).unwrap_err() else {
                panic!("expected Error");
            };
            assert_eq!(error.name, "TypeError");
            assert_eq!(error.message, "Invalid value used as weak map key");
        }
        schema.replace(json!({}), BTreeMap::new(), false);
        assert_eq!(validate(schema, json!({})).unwrap(), json!({}));
    }
}

#[test]
fn complete_argument_path_handles_measured_deep_inputs() {
    fn discard(mut values: Vec<Value>) {
        while let Some(value) = values.pop() {
            match value {
                Value::Array(items) => values.extend(items),
                Value::Object(items) => values.extend(items.into_values()),
                _ => {}
            }
        }
    }
    for depth in [128, 256, 449, 450] {
        let input = format!("{}\"42\"{}", "[".repeat(depth), "]".repeat(depth));
        let parsed = parse_json_with_repair(&input).unwrap();
        assert_eq!(repair_json(&input), input);
        let mut schema = json!({"type":"integer"});
        for _ in 0..depth {
            schema = json!({"type":"array","items":schema});
        }
        let schema: TSchema = schema.into();
        let result = validate(schema.clone(), parsed.clone()).unwrap();
        let mut leaf = &result;
        for _ in 0..depth {
            leaf = &leaf[0];
        }
        assert_eq!(*leaf, json!(42));
        discard(vec![parsed, result]);
        drop(schema);
    }
    for depth in [3291, 3292] {
        let input = format!("{}\"42\"{}", "[".repeat(depth), "]".repeat(depth));
        let parsed = parse_json_with_repair(&input).unwrap();
        let call = call(parsed);
        let result = validate_tool_arguments(&tool(json!({})), &call).unwrap();
        let error = text(validate_tool_arguments(&tool(json!({"not":{}})), &call).unwrap_err());
        let mut expected = String::from(
            "Validation failed for tool \"echo\":\n  - root: must not be valid\n\nReceived arguments:\n",
        );
        for i in 0..depth {
            expected.push_str(&"  ".repeat(i));
            expected.push_str("[\n");
        }
        expected.push_str(&"  ".repeat(depth));
        expected.push_str("\"42\"\n");
        for i in (0..depth).rev() {
            expected.push_str(&"  ".repeat(i));
            expected.push(']');
            if i > 0 {
                expected.push('\n');
            }
        }
        assert_eq!(error, expected);
        discard(vec![call.arguments, result]);
    }
}

#[test]
fn metadata_conversion_and_checker_keep_distinct_regex_modes() {
    let schema = |pattern: &str| {
        metadata(
            json!({"type":"object","patternProperties":{pattern:{"type":"number"}}}),
            &[(&[], "Record"), (&["patternProperties", pattern], "Number")],
            true,
        )
    };
    assert_eq!(
        validate(schema("^..$"), json!({"😀":""})).unwrap(),
        json!({"😀":0})
    );
    assert_eq!(
        text(validate(schema("^.$"), json!({"😀":""})).unwrap_err()),
        "Validation failed for tool \"echo\":\n  - 😀: must be number\n\nReceived arguments:\n{\n  \"😀\": \"\"\n}"
    );
}

#[test]
fn primitive_round_trip_coercion_still_runs_final_validation() {
    assert_eq!(
        text(
            validate(
                json!({"allOf":[{"type":"string"},{"type":"boolean"}]}),
                json!(true)
            )
            .unwrap_err()
        ),
        "Validation failed for tool \"echo\":\n  - root: must be string\n\nReceived arguments:\ntrue"
    );
}

#[test]
fn argument_path_tears_down_deep_temporaries_in_subprocess() {
    const MODE: &str = "MAESTRO_DEEP_ARGUMENT_MODE";
    let Ok(mode) = std::env::var(MODE) else {
        for mode in ["valid", "error"] {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .arg("--exact")
                .arg("argument_path_tears_down_deep_temporaries_in_subprocess")
                .arg("--nocapture")
                .env(MODE, mode)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{mode}: {}\n{}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            );
        }
        return;
    };
    let depth = 100_000;
    let input = format!("{}0{}", "[".repeat(depth), "]".repeat(depth));
    let call = call(parse_json_with_repair(&input).unwrap());
    let result = validate_tool_arguments(
        &tool(if mode == "valid" {
            json!({})
        } else {
            json!(false)
        }),
        &call,
    );
    let mut pending = vec![call.arguments];
    if mode == "valid" {
        let value = result.unwrap();
        let mut leaf = &value;
        for _ in 0..depth {
            leaf = &leaf[0];
        }
        assert_eq!(leaf, &json!(0));
        pending.push(value);
    } else {
        let ThrownValue::Error(error) = result.unwrap_err() else {
            panic!("expected Error");
        };
        assert_eq!(error.name, "TypeError");
        assert_eq!(error.message, "Invalid value used as weak map key");
    }
    while let Some(value) = pending.pop() {
        match value {
            Value::Array(items) => pending.extend(items),
            Value::Object(items) => pending.extend(items.into_values()),
            _ => {}
        }
    }
}

#[test]
fn array_object_unions_keep_interpreted_acceptance() {
    // Qualified library difference: the reference compiled accelerator rejects this,
    // while live Errors is empty and the reference reports "Unknown validation error".
    for schema in [
        json!({"type":["array","object"],"properties":{"0":{"type":"number"}}}),
        json!({"type":["array","object"],"additionalProperties":{"type":"number"}}),
        json!({"type":["array","object"],"properties":{"1":{"type":"boolean"}},"additionalProperties":{"type":"number"}}),
    ] {
        let input = json!(["42", "true"]);
        assert_eq!(validate(schema, input.clone()).unwrap(), input);
    }
}
