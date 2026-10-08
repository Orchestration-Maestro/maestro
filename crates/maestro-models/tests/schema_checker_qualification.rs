use maestro_models::{Tool, ToolCall, validate_tool_arguments};
use serde_json::{Value, json};

fn check(schema: Value, input: Value) -> Result<Value, String> {
    let parameters = Value::Object(maestro_models::JsonObject::from_iter([
        ("type".into(), json!("object")),
        (
            "properties".into(),
            Value::Object(maestro_models::JsonObject::from_iter([(
                "value".into(),
                schema,
            )])),
        ),
    ]));
    check_object(
        parameters,
        maestro_models::JsonObject::from_iter([("value".into(), input)]),
    )
    .map(|arguments| arguments["value"].clone())
}

fn check_object(
    schema: Value,
    arguments: maestro_models::JsonObject,
) -> Result<maestro_models::JsonObject, String> {
    let tool = Tool {
        name: "check".into(),
        description: String::new(),
        parameters: schema,
    };
    let call = ToolCall {
        id: "1".into(),
        name: "check".into(),
        arguments,
        thought_signature: None,
    };
    validate_tool_arguments(&tool, &call).map_err(|error| {
        assert_eq!(error.name.as_deref(), Some("Error"));
        assert_eq!((error.stack, error.code), (None, None));
        error.message
    })
}

#[test]
fn maestro_validates_scalar_constraints_and_numeric_multiples() {
    for (schema, valid, invalid, message) in [
        (json!({"maximum":2}), json!(2), json!(3), "must be <= 2"),
        (json!({"minimum":2}), json!(2), json!(1), "must be >= 2"),
        (
            json!({"exclusiveMaximum":2}),
            json!(1),
            json!(2),
            "must be < 2",
        ),
        (
            json!({"exclusiveMinimum":2}),
            json!(3),
            json!(2),
            "must be > 2",
        ),
        (
            json!({"multipleOf":0.1}),
            json!(0.3),
            json!(0.31),
            "must be multiple of 0.1",
        ),
        (
            json!({"const":{"a":1,"b":2}}),
            json!({"b":2,"a":1}),
            json!({"a":1}),
            "must be equal to constant",
        ),
        (
            json!({"enum":[1,"a"]}),
            json!("a"),
            json!(true),
            "must be equal to one of the allowed values",
        ),
    ] {
        assert_eq!(check(schema.clone(), valid.clone()).unwrap(), valid);
        assert_eq!(
            check(schema, invalid.clone()).unwrap_err(),
            format!(
                "Validation failed for tool \"check\":\n  - value: {message}\n\nReceived arguments:\n{}",
                serde_json::to_string_pretty(&json!({"value":invalid})).unwrap()
            )
        );
    }
    assert!(check(json!({"multipleOf":0}), json!(1)).is_err());
    assert!(check(json!({"type":"unknown"}), json!(1)).is_ok());
    assert!(check(json!({"type":"bigint"}), json!(1)).is_err());
    assert!(check(json!({"minLength":1.5}), json!("a")).is_err());
    assert!(check(json!({"maxLength":-1}), json!("")).is_err());
    assert!(check(json!({"type":[]}), json!(1)).is_err());
    assert!(check(json!({"type":["string",1]}), json!(1)).is_ok());
    assert!(check(json!({"allOf":[{"type":"number"},0]}), json!("bad")).is_ok());
}

#[test]
fn maestro_validates_collection_keywords_without_extra_coercion() {
    for (schema, accepted, rejected, message) in object_cases().into_iter().chain(array_cases()) {
        assert_eq!(check(schema.clone(), accepted.clone()).unwrap(), accepted);
        assert_eq!(
            check(schema, rejected.clone()).unwrap_err(),
            format!(
                "Validation failed for tool \"check\":\n  - value: {message}\n\nReceived arguments:\n{}",
                serde_json::to_string_pretty(&json!({"value":rejected})).unwrap()
            )
        );
    }
    for (schema, input) in accepted_collection_edges() {
        assert!(check(schema, input).is_ok());
    }
    for (schema, input) in rejected_collection_edges() {
        assert!(check(schema, input).is_err());
    }
    assert_eq!(check(json!({"items":[true],"additionalItems":false}), json!([1,2,3])),
        Err("Validation failed for tool \"check\":\n  - value.1: schema is false\n  - value.2: schema is false\n\nReceived arguments:\n{\n  \"value\": [\n    1,\n    2,\n    3\n  ]\n}".into()),
        "additional_items_tail_reports_all_failures");
}

#[test]
fn maestro_merges_evaluated_locations_across_schema_branches() {
    let schema = json!({"anyOf":[{"properties":{"a":true}},{"properties":{"b":true}}],"unevaluatedProperties":false});
    assert!(check(schema.clone(), json!({"a":1,"b":2})).is_ok());
    assert!(check(schema, json!({"a":1,"c":2})).is_err());
    assert!(
        check(
            json!({"allOf":[{"properties":{"a":true}}],"unevaluatedProperties":false}),
            json!({"a":1})
        )
        .is_ok()
    );
    assert!(check(json!({"oneOf":[{"required":["a"],"properties":{"a":true}},{"required":["b"],"properties":{"b":true}}],"unevaluatedProperties":false}), json!({"a":1})).is_ok());
    assert!(
        check(
            json!({"prefixItems":[true],"unevaluatedItems":false}),
            json!([1])
        )
        .is_ok()
    );
    assert!(
        check(
            json!({"prefixItems":[true],"unevaluatedItems":false}),
            json!([1, 2])
        )
        .is_err()
    );
    assert!(check(json!({"if":{"required":["a"]},"then":{"properties":{"a":true}},"else":{"properties":{"b":true}},"unevaluatedProperties":false}), json!({"a":1})).is_ok());
    assert!(check(json!({"if":{"required":["a"]},"then":{"properties":{"a":true}},"else":{"properties":{"b":true}},"unevaluatedProperties":false}), json!({"b":2})).is_ok());
    assert!(check(json!({"not":{"type":"number"}}), json!("1")).is_ok());
    assert!(check(json!({"not":{"type":"number"}}), json!(1)).is_err());
}

fn object_cases() -> Vec<(Value, Value, Value, &'static str)> {
    vec![
        (
            json!({"additionalProperties":false}),
            json!({}),
            json!({"x":1}),
            "must not have additional properties",
        ),
        (
            json!({"minProperties":1}),
            json!({"x":1}),
            json!({}),
            "must not have fewer than 1 properties",
        ),
        (
            json!({"maxProperties":1}),
            json!({"x":1}),
            json!({"x":1,"y":2}),
            "must not have more than 1 properties",
        ),
    ]
}

fn array_cases() -> Vec<(Value, Value, Value, &'static str)> {
    vec![
        (
            json!({"minItems":1}),
            json!([1]),
            json!([]),
            "must not have fewer than 1 items",
        ),
        (
            json!({"maxItems":1}),
            json!([1]),
            json!([1, 2]),
            "must not have more than 1 items",
        ),
        (
            json!({"uniqueItems":true}),
            json!([{"a":1},{"a":2}]),
            json!([{"a":1,"b":2},{"b":2,"a":1}]),
            "must not have duplicate items",
        ),
        (
            json!({"contains":{"type":"number"}}),
            json!([1]),
            json!(["1"]),
            "must contain at least 1 valid item",
        ),
        (
            json!({"dependentRequired":{"a":["b"]}}),
            json!({"a":1,"b":2}),
            json!({"a":1}),
            "must have properties b when property a is present",
        ),
        (
            json!({"dependencies":{"a":["b"]}}),
            json!({"a":1,"b":2}),
            json!({"a":1}),
            "must have properties b when property a is present",
        ),
    ]
}

#[test]
fn maestro_preserves_reference_format_acceptance() {
    for (format, accepted, rejected) in FORMAT_CASES {
        for text in accepted {
            assert_eq!(
                check(json!({"format":format}), json!(text)),
                Ok(json!(text)),
                "{format}: {text}"
            );
        }
        for text in rejected {
            assert_eq!(
                check(json!({"format":format}), json!(text)),
                Err(format!(
                    "Validation failed for tool \"check\":\n  - value: must match format \"{format}\"\n\nReceived arguments:\n{}",
                    serde_json::to_string_pretty(&json!({"value":text})).unwrap()
                )),
                "{format}: {text}"
            );
        }
    }
    for (format, input, valid) in format_edges() {
        assert_eq!(
            check(json!({"format":format}), input).is_ok(),
            valid,
            "{format}"
        );
    }
}

type FormatCase = (&'static str, [&'static str; 2], [&'static str; 2]);
const FORMAT_CASES: &[FormatCase] = &[
    (
        "date-time",
        ["2024-02-29t12:00:00z", "2016-12-31T23:59:60Z"],
        ["2023-02-29T12:00:00Z", "2024-02-29 12:00:00Z"],
    ),
    (
        "date",
        ["0000-02-29", "2000-02-29"],
        ["1900-02-29", "2024-2-29"],
    ),
    (
        "time",
        ["23:59:60Z", "00:59:60+01:00"],
        ["12:00:00", "24:00:00Z"],
    ),
    ("duration", ["P1Y2M3DT4H5M6S", "P2W"], ["P", "P1.5D"]),
    (
        "email",
        ["a@b", "A.B@example.com"],
        ["\"a\"@example.com", "é@example.com"],
    ),
    (
        "idn-email",
        ["é@例子.测试", "a@b"],
        ["a..b@example.com", "\"é\"@example.com"],
    ),
    (
        "hostname",
        ["example.com", "xn--mnchen-3ya.example"],
        ["example.com.", "-x.example"],
    ),
    (
        "idn-hostname",
        ["münchen。example", "l·l.example"],
        ["a·b.example", "́a.example"],
    ),
    (
        "ipv4",
        ["0.0.0.0", "255.255.255.255"],
        ["01.2.3.4", "256.2.3.4"],
    ),
    (
        "ipv6",
        ["::", "::ffff:192.0.2.1"],
        ["1::2::3", "fe80::1%eth0"],
    ),
    (
        "uri",
        ["urn:example:test", "https://例子.example/a"],
        ["/relative", "http://example/a%Q0"],
    ),
    ("uri-reference", ["", "../a?b#c"], ["é", "a b"]),
    (
        "iri",
        ["https://example/é", "urn:example:é"],
        ["/relative", "http://[broken]/"],
    ),
    (
        "iri-reference",
        ["", "../é"],
        ["a%Q0", "httpx//example.com"],
    ),
    ("uri-template", ["/x/{id}{?q}", "{+path}"], ["{x:0}", "{x"]),
    (
        "url",
        ["https://example.com/", "ftp://8.8.8.8/"],
        ["http://127.0.0.1/", "http://localhost/"],
    ),
    (
        "uuid",
        [
            "00000000-0000-0000-0000-000000000000",
            "URN:UUID:550e8400-e29b-41d4-a716-446655440000",
        ],
        ["550e8400e29b41d4a716446655440000", "bad"],
    ),
    ("regex", ["a+", "\\p{L}"], ["[", ""]),
    ("json-pointer", ["", "/a~1b/~0"], ["a", "/~2"]),
    (
        "json-pointer-uri-fragment",
        ["#", "#/a%20b/~0"],
        ["/a", "#/~2"],
    ),
    ("relative-json-pointer", ["0", "2/a~1b"], ["01/a", "-1/a"]),
];

#[test]
fn maestro_rejects_ipv6_with_a_trailing_single_colon() {
    {
        let text = "1:2:3:4:5:6:7:8";
        assert_eq!(
            check(json!({"format":"ipv6"}), json!(text)),
            Ok(json!(text))
        );
    }
    {
        let text = "1:2:3:4:5:6:7:8:";
        assert_eq!(
            check(json!({"format":"ipv6"}), json!(text)),
            Err(format!(
                "Validation failed for tool \"check\":\n  - value: must match format \"ipv6\"\n\nReceived arguments:\n{}",
                serde_json::to_string_pretty(&json!({"value":text})).unwrap()
            ))
        );
    }
}

#[test]
fn maestro_rejects_invalid_local_leap_second_times() {
    for text in ["24:59:60+01:00", "23:60:60Z"] {
        assert_eq!(
            check(json!({"format":"time"}), json!(text)),
            Err(format!(
                "Validation failed for tool \"check\":\n  - value: must match format \"time\"\n\nReceived arguments:\n{}",
                serde_json::to_string_pretty(&json!({"value":text})).unwrap()
            ))
        );
    }
}

#[test]
fn maestro_rejects_invalid_internationalized_host_labels() {
    for text in ["münchen.example", "xn--mnchen-3ya.example"] {
        assert_eq!(
            check(json!({"format":"idn-hostname"}), json!(text)),
            Ok(json!(text))
        );
    }
    for text in ["a_b.example", "a!b.example", "\t.example", "L·L.example"] {
        assert_eq!(
            check(json!({"format":"idn-hostname"}), json!(text)),
            Err(format!(
                "Validation failed for tool \"check\":\n  - value: must match format \"idn-hostname\"\n\nReceived arguments:\n{}",
                serde_json::to_string_pretty(&json!({"value":text})).unwrap()
            ))
        );
    }
}

#[test]
fn maestro_validates_bracketed_uri_literals() {
    for text in ["http://[::1]/", "http://[v1.ab:cd]/"] {
        assert_eq!(check(json!({"format":"uri"}), json!(text)), Ok(json!(text)));
    }
    {
        let text = "http://[not-an-ip]/";
        assert_eq!(
            check(json!({"format":"uri"}), json!(text)),
            Err(format!(
                "Validation failed for tool \"check\":\n  - value: must match format \"uri\"\n\nReceived arguments:\n{}",
                serde_json::to_string_pretty(&json!({"value":text})).unwrap()
            ))
        );
    }
}

#[test]
fn maestro_reports_actual_contains_bounds() {
    for (schema, input, message) in [
        (
            json!({"contains":true,"minContains":2}),
            json!([1]),
            "must contain at least 2 valid items",
        ),
        (
            json!({"contains":false}),
            json!([1]),
            "must contain at least 1 valid item",
        ),
    ] {
        assert_eq!(
            check(schema, input.clone()),
            Err(format!(
                "Validation failed for tool \"check\":\n  - value: {message}\n\nReceived arguments:\n{}",
                serde_json::to_string_pretty(&json!({"value":input})).unwrap()
            ))
        );
    }
}

#[test]
fn maestro_keeps_schema_resource_origins_distinct() {
    for (reference, valid) in [
        ("https://one.example/schema", true),
        ("https://two.example/schema", false),
        ("https://one.example/schema?q=1", false),
    ] {
        let schema = json!({"$id":"https://one.example/schema", "$defs":{"target":{"$id":"target", "type":"number"}},"properties":{"n":{"$ref":format!("{reference}#/$defs/target")}}});
        let tool = Tool {
            name: "check".into(),
            description: String::new(),
            parameters: schema,
        };
        let call = ToolCall {
            id: "1".into(),
            name: "check".into(),
            arguments: maestro_models::JsonObject::from_iter([("n".into(), json!(1))]),
            thought_signature: None,
        };
        if valid {
            assert_eq!(validate_tool_arguments(&tool, &call).unwrap()["n"], 1);
        } else {
            assert_eq!(
                validate_tool_arguments(&tool, &call).unwrap_err().message,
                "Validation failed for tool \"check\":\n  - n: schema is false\n\nReceived arguments:\n{\n  \"n\": 1\n}"
            );
        }
    }
}

#[test]
fn maestro_rejects_nonprogressing_reference_cycles() {
    for schema in [
        json!({"$id":"https://cycles.example/","$ref":"#"}),
        json!({"$id":"https://cycles.example/","$defs":{"a":{"$ref":"#/$defs/b"},"b":{"$ref":"#/$defs/a"}},"$ref":"#/$defs/a"}),
    ] {
        assert_eq!(check(schema, json!({})), Err("Validation failed for tool \"check\":\n  - value: schema is false\n\nReceived arguments:\n{\n  \"value\": {}\n}".into()));
    }
    let schema = json!({"$id":"https://cycles.example/","$defs":{"node":{"properties":{"next":{"$ref":"#/$defs/node"},"value":{"type":"number"}}}},"$ref":"#/$defs/node"});
    assert!(
        check(
            schema.clone(),
            json!({"value":1,"next":{"value":2,"next":{"value":3}}})
        )
        .is_ok()
    );
    assert!(check(schema, json!({"value":1,"next":{"value":"2"}})).is_err());
    assert!(check(json!({"$id":"https://cycles.example/","$defs":{"n":{"type":"number"}},"properties":{"a":{"$ref":"#/$defs/n"},"b":{"$ref":"#/$defs/n"}}}), json!({"a":1,"b":2})).is_ok());
}

#[test]
fn maestro_resolves_schema_ids_anchors_and_recursive_references() {
    let arguments = maestro_models::JsonObject::from_iter([("example".into(), json!(1))]);
    assert_eq!(
        check_object(json!({"propertyNames":{"$ref":"#"}}), arguments.clone()).unwrap(),
        arguments
    );
    let schema = json!({"$id":"https://schemas.example/root","$defs":{"n":{"$id":"number","$anchor":"count","type":"number"}},"properties":{"a":{"$ref":"number#count"},"b":{"$ref":"https://schemas.example/number"}}});
    let arguments =
        maestro_models::JsonObject::from_iter([("a".into(), json!(1)), ("b".into(), json!(2))]);
    assert_eq!(check_object(schema, arguments.clone()).unwrap(), arguments);
    let recursive = json!({"$id":"https://schemas.example/node","$recursiveAnchor":true,"properties":{"value":{"type":"number"},"next":{"$recursiveRef":"#"}}});
    assert!(check(recursive.clone(), json!({"value":1,"next":{"value":2}})).is_ok());
    assert!(check(recursive, json!({"value":1,"next":{"value":"2"}})).is_err());
    let dynamic = json!({"$id":"https://schemas.example/dynamic","$dynamicAnchor":"node","properties":{"value":{"type":"number"},"next":{"$dynamicRef":"#node"}}});
    assert!(check(dynamic.clone(), json!({"value":1,"next":{"value":2}})).is_ok());
    assert!(check(dynamic, json!({"value":1,"next":{"value":"2"}})).is_err());
    let sibling = json!({"$id":"https://schemas.example/sibling","$defs":{"n":{"type":"number"}},"$ref":"#/$defs/n","minimum":3});
    assert!(check(sibling.clone(), json!(3)).is_ok());
    assert!(check(sibling, json!(2)).is_err());
    let overrides = json!({
        "$id":"https://schemas.example/outer", "$dynamicAnchor":"node",
        "properties":{"value":{"type":"number"},"next":{"$ref":"https://schemas.example/inner"}},
        "$defs":{"inner":{"$id":"inner", "$dynamicAnchor":"node",
            "properties":{"value":{"type":"string"},"next":{"$dynamicRef":"#node"}}}}
    });
    assert!(
        check(
            overrides.clone(),
            json!({"value":1,"next":{"value":"inner","next":{"value":2}}})
        )
        .is_ok()
    );
    assert!(
        check(
            overrides,
            json!({"value":1,"next":{"value":"inner","next":{"value":"wrong"}}})
        )
        .is_err()
    );
    let pointer = json!({"$id":"https://schemas.example/pointer", "$dynamicAnchor":"node",
        "$defs":{"string":{"$dynamicAnchor":"node","type":"string"}},
        "properties":{"value":{"$dynamicRef":"#/$defs/string"}}});
    assert!(check(pointer.clone(), json!({"value":"text"})).is_ok());
    assert!(check(pointer, json!({"value":1})).is_err());
}

fn accepted_collection_edges() -> Vec<(Value, Value)> {
    vec![
        (
            json!({"properties":{"a":{"type":"number"},"b":0}}),
            json!({"a":"unchanged"}),
        ),
        (json!({"required":["missing",1]}), json!({})),
        (
            json!({"dependencies":{"a":{"required":["b"]}}}),
            json!({"a":1,"b":2}),
        ),
        (
            json!({"propertyNames":{"pattern":"^[a-z]+$"}}),
            json!({"good":1}),
        ),
        (json!({"items":[true],"additionalItems":false}), json!([1])),
        (json!({"contains":false,"minContains":0}), json!([])),
        (json!({"minContains":3,"maxContains":0}), json!([1])),
        (
            json!({"patternProperties":{"^\\p{L}+$":{"type":"number"}},"additionalProperties":false}),
            json!({"é":1}),
        ),
    ]
}

fn rejected_collection_edges() -> Vec<(Value, Value)> {
    vec![
        (
            json!({"propertyNames":{"pattern":"^[a-z]+$"}}),
            json!({"wrong key":1}),
        ),
        (
            json!({"items":[true],"additionalItems":false}),
            json!([1, 2]),
        ),
        (json!({"minItems":1.5}), json!([1])),
        (json!({"maxItems":-1}), json!([])),
        (json!({"minProperties":1.5}), json!({"a":1})),
        (json!({"contains":true,"minContains":1.5}), json!([1])),
    ]
}

fn format_edges() -> Vec<(&'static str, Value, bool)> {
    vec![
        ("uri", json!("urn:test\n"), false),
        ("uri", json!("http://user\n@example.com/"), false),
        ("hostname", json!("example\n"), false),
        (
            "hostname",
            json!(format!("{}.example", "a".repeat(63))),
            true,
        ),
        (
            "hostname",
            json!(format!("{}.example", "a".repeat(64))),
            false,
        ),
        ("hostname", json!("ab--cd.example"), false),
        ("idn-hostname", json!("͵α.example"), true),
        ("idn-hostname", json!("͵a.example"), false),
        ("idn-hostname", json!("א׳.example"), true),
        ("idn-hostname", json!("a׳.example"), false),
        ("idn-hostname", json!("カ・ナ.example"), true),
        ("idn-hostname", json!("a・b.example"), false),
        ("idn-hostname", json!("١۲.example"), false),
        ("idn-hostname", json!("a‍b.example"), false),
        ("uri", json!("http://example:999999/"), true),
        ("url", json!("http://example.com:1/"), false),
        ("url", json!("http://example.com:123456/"), false),
        ("url", json!("http://10.0.0.1/"), false),
        ("uri-reference", json!("a%Q0"), true),
        ("date", json!("2000-02-29\n"), false),
        ("hostname", json!(42), true),
        ("unregistered", json!("anything"), true),
    ]
}

#[test]
fn maestro_checks_fractional_multiples_independently_of_sign() {
    for (divisor, valid_values) in [
        (0.1, &[-0.3, 0.300_000_000_05, -0.300_000_000_05][..]),
        (-0.1, &[0.3, -0.3, 0.300_000_000_05, -0.300_000_000_05][..]),
    ] {
        for valid in valid_values {
            assert_eq!(
                check(json!({"multipleOf":divisor}), json!(valid)).unwrap(),
                json!(valid)
            );
        }
        for invalid in [0.300_000_000_2, -0.300_000_000_2] {
            assert!(check(json!({"multipleOf":divisor}), json!(invalid)).is_err());
        }
    }
}

#[test]
fn maestro_resolves_nonfragment_recursive_refs_from_active_resource() {
    let schema = json!({
        "$id":"https://schemas.example/root/",
        "$defs":{"node":{"$id":"https://schemas.example/node/", "$recursiveRef":"leaf", "$defs":{"leaf":{"$id":"leaf", "type":"number"}}}},
        "$ref":"https://schemas.example/node/"
    });
    assert_eq!(check(schema.clone(), json!(3)).unwrap(), json!(3));
    assert!(
        check(schema, json!("wrong"))
            .unwrap_err()
            .contains("must be number")
    );
}

#[test]
fn maestro_never_dynamically_overrides_encoded_pointer_fragments() {
    let schema = json!({
        "$id":"https://schemas.example/root", "$dynamicAnchor":"kind", "type":"number",
        "$defs":{"nested":{"$id":"https://schemas.example/nested", "$dynamicRef":"#%2F$defs%2Fstring", "$defs":{"string":{"$dynamicAnchor":"kind", "type":"string"}}}},
        "$ref":"https://schemas.example/nested"
    });
    let error = check(schema, json!(3)).unwrap_err();
    assert!(error.contains("must be string"), "{error}");
    let schema = json!({"$id":"https://schemas.example/plain", "$defs":{"string":{"type":"string"}}, "$dynamicRef":"#%2F$defs%2Fstring"});
    assert_eq!(check(schema.clone(), json!("text")).unwrap(), json!("text"));
    assert!(check(schema, json!(3)).is_err());
}

#[test]
fn maestro_reports_all_declared_dependency_names() {
    for keyword in ["dependencies", "dependentRequired"] {
        let schema = json!({keyword:{"a":["b","c"]}});
        let input =
            maestro_models::JsonObject::from_iter([("a".into(), json!(1)), ("b".into(), json!(2))]);
        assert_eq!(
            check_object(schema, input).unwrap_err(),
            "Validation failed for tool \"check\":\n  - root: must have properties b, c when property a is present\n\nReceived arguments:\n{\n  \"a\": 1,\n  \"b\": 2\n}"
        );
    }
}

#[test]
fn maestro_preserves_literal_missing_property_names() {
    for (schema, input, path) in [
        (json!({"required":["a/b"]}), json!({}), "a/b"),
        (
            json!({"properties":{"nested":{"required":["a/b"]}}}),
            json!({"nested":{}}),
            "nested.a/b",
        ),
    ] {
        assert_eq!(
            check_object(schema, input.as_object().unwrap().clone()).unwrap_err(),
            format!(
                "Validation failed for tool \"check\":\n  - {path}: must have required properties a/b\n\nReceived arguments:\n{}",
                serde_json::to_string_pretty(&input).unwrap()
            )
        );
    }
}

#[test]
fn maestro_preserves_inherited_uris_for_relative_schema_resources() {
    let schema = json!({
        "$id":"https://schemas.example/root/",
        "$defs":{"nested":{"$id":"nested", "$ref":"#/$defs/string", "$defs":{"string":{"type":"string"}}}},
        "$ref":"nested"
    });
    assert_eq!(check(schema.clone(), json!("text")).unwrap(), json!("text"));
    assert!(
        check(schema, json!(3))
            .unwrap_err()
            .contains("must be string")
    );
}

#[test]
fn maestro_validates_lookaround_backreferences_and_astral_patterns() {
    for (pattern, valid, invalid) in [
        (r"^a(?=b)b$", "ab", "ac"),
        (r"(?<=a)b$", "ab", "cb"),
        (r"^(a)\1$", "aa", "ab"),
        (r"^.$", "😀", "😀😀"),
        (r"^\u{1F600}$", "😀", "😁"),
    ] {
        assert_eq!(
            check(json!({"pattern":pattern}), json!(valid)).unwrap(),
            json!(valid)
        );
        assert_eq!(
            check(json!({"pattern":pattern}), json!(invalid)),
            Err(format!(
                "Validation failed for tool \"check\":\n  - value: must match pattern \"{pattern}\"\n\nReceived arguments:\n{}",
                serde_json::to_string_pretty(&json!({"value":invalid})).unwrap()
            ))
        );
        let schema = json!({"patternProperties":{pattern:false}});
        assert!(check(schema.clone(), json!({valid:1})).is_err());
        assert_eq!(
            check(schema, json!({invalid:1})).unwrap(),
            json!({invalid:1})
        );
    }
}

#[test]
fn maestro_checks_whole_hostname_lengths_and_valid_joiner_context() {
    for format in ["hostname", "idn-hostname"] {
        let valid = [
            "a".repeat(63),
            "b".repeat(63),
            "c".repeat(63),
            "d".repeat(61),
        ]
        .join(".");
        let invalid = format!("{valid}d");
        assert_eq!(
            check(json!({"format":format}), json!(valid)).unwrap(),
            json!(valid)
        );
        assert!(check(json!({"format":format}), json!(invalid)).is_err());
    }
    for valid in ["क्‍ष.example", "क्‌ष.example"] {
        assert_eq!(
            check(json!({"format":"idn-hostname"}), json!(valid)).unwrap(),
            json!(valid)
        );
    }
    for invalid in ["क‍ष.example", "क‌ष.example"] {
        assert!(check(json!({"format":"idn-hostname"}), json!(invalid)).is_err());
    }
}

#[test]
fn maestro_uses_ordinary_paths_for_empty_required_names() {
    for (schema, input, path) in [
        (json!({"required":[""]}), json!({}), "root"),
        (
            json!({"properties":{"nested":{"required":[""]}}}),
            json!({"nested":{}}),
            "nested",
        ),
    ] {
        assert_eq!(
            check_object(schema, input.as_object().unwrap().clone()).unwrap_err(),
            format!(
                "Validation failed for tool \"check\":\n  - {path}: must have required properties \n\nReceived arguments:\n{}",
                serde_json::to_string_pretty(&input).unwrap()
            )
        );
    }
}

#[test]
fn maestro_resolves_fragment_id_aliases_for_every_reference_kind() {
    for keyword in ["$ref", "$recursiveRef", "$dynamicRef"] {
        let schema = json!({"$defs":{"n":{"$id":"#num","type":"number"}},"properties":{"n":{keyword:"#num"}}});
        assert_eq!(
            check_object(schema.clone(), json!({"n":1}).as_object().unwrap().clone()).unwrap(),
            json!({"n":1}).as_object().unwrap().clone()
        );
        assert_eq!(
            check_object(schema, json!({"n":"x"}).as_object().unwrap().clone()).unwrap_err(),
            "Validation failed for tool \"check\":\n  - n: must be number\n\nReceived arguments:\n{\n  \"n\": \"x\"\n}"
        );
    }
}

fn assert_outcomes(
    schema: Value,
    accepted: Value,
    rejected: Value,
    diagnostics: &str,
) -> Result<(), serde_json::Error> {
    assert_eq!(check(schema.clone(), accepted.clone()), Ok(accepted));
    let original = serde_json::to_string_pretty(&json!({"value":rejected}))?;
    assert_eq!(
        check(schema, rejected),
        Err(format!(
            "Validation failed for tool \"check\":\n{diagnostics}\n\nReceived arguments:\n{original}"
        ))
    );
    Ok(())
}

#[test]
fn maestro_checks_schema_valued_additional_and_unevaluated_members() {
    for (schema, accepted, rejected, diagnostics) in [
        (
            json!({"items":[true],"additionalItems":{"type":"number"}}),
            json!(["head", 2]),
            json!(["head", "bad"]),
            "  - value.1: must be number",
        ),
        (
            json!({"additionalProperties":{"type":"number"}}),
            json!({"n":2}),
            json!({"n":"bad"}),
            "  - value: must not have additional properties",
        ),
        (
            json!({"prefixItems":[true],"unevaluatedItems":{"type":"number"}}),
            json!(["head", 2]),
            json!(["head", "bad"]),
            "  - value: must not have unevaluated items",
        ),
        (
            json!({"properties":{"head":true},"unevaluatedProperties":{"type":"number"}}),
            json!({"head":"head","n":2}),
            json!({"head":"head","n":"bad"}),
            "  - value: must not have unevaluated properties",
        ),
    ] {
        assert_outcomes(schema, accepted, rejected, diagnostics).unwrap();
    }
}

#[test]
fn maestro_accepts_dependent_schemas_and_active_contains_maximum() {
    assert_outcomes(
        json!({"dependentSchemas":{"a":{"required":["b"]}}}),
        json!({"a":1,"b":2}),
        json!({"a":1}),
        "  - value.b: must have required properties b",
    )
    .unwrap();
    assert_outcomes(
        json!({"contains":true,"maxContains":1}),
        json!([1]),
        json!([1, 2]),
        "  - value: must contain at most 1 valid item",
    )
    .unwrap();
}

#[test]
fn maestro_resolves_pointers_inside_fragment_id_scopes() {
    for keyword in ["$ref", "$recursiveRef", "$dynamicRef"] {
        let schema = json!({"$defs":{"n":{"$id":"#num","$defs":{"v":{"type":"number"}},keyword:"#/$defs/v"}},"properties":{"n":{"$ref":"#num"}}});
        let accepted = json!({"n":1}).as_object().unwrap().clone();
        assert_eq!(check_object(schema.clone(), accepted.clone()), Ok(accepted));
        assert_eq!(
            check_object(schema, json!({"n":"x"}).as_object().unwrap().clone()).unwrap_err(),
            "Validation failed for tool \"check\":\n  - n: must be number\n\nReceived arguments:\n{\n  \"n\": \"x\"\n}"
        );
    }
}

#[test]
fn maestro_requires_exact_array_pointer_indexes() {
    for keyword in ["$ref", "$recursiveRef", "$dynamicRef"] {
        for (index, tuple) in [
            ("0", json!([{"type":"number"}, false])),
            ("1", json!([false, {"type":"number"}])),
        ] {
            let schema = json!({"$defs":{"tuple":tuple},"properties":{"n":{keyword:format!("#/$defs/tuple/{index}")}}});
            let accepted = json!({"n":1}).as_object().unwrap().clone();
            assert_eq!(check_object(schema, accepted.clone()), Ok(accepted));
        }
        for index in ["+0", "+1", "%2B0", "%2B1", "01", "-0"] {
            let schema = json!({"$defs":{"tuple":[{"type":"number"},{"type":"number"}]},"properties":{"n":{keyword:format!("#/$defs/tuple/{index}")}}});
            assert_eq!(
                check_object(schema, json!({"n":1}).as_object().unwrap().clone()).unwrap_err(),
                "Validation failed for tool \"check\":\n  - n: schema is false\n\nReceived arguments:\n{\n  \"n\": 1\n}"
            );
        }
    }
}

#[test]
fn maestro_limits_ordered_diagnostics_to_eight_distinct_messages() {
    let schema = json!({"properties":{
        "a":{"type":"number"},"b":{"type":"number"},"c":{"type":"number"},
        "d":{"type":"number"},"e":{"type":"number"},"f":{"type":"number"},
        "g":{"type":"number"},"h":{"type":"number"},"i":{"type":"number"}
    }});
    let arguments = json!({"a":{},"b":{},"c":{},"d":{},"e":{},"f":{},"g":{},"h":{},"i":{}});
    assert_eq!(
        check_object(schema, arguments.as_object().unwrap().clone()).unwrap_err(),
        format!(
            "Validation failed for tool \"check\":\n  - a: must be number\n  - b: must be number\n  - c: must be number\n  - d: must be number\n  - e: must be number\n  - f: must be number\n  - g: must be number\n  - h: must be number\n\nReceived arguments:\n{}",
            serde_json::to_string_pretty(&arguments).unwrap()
        )
    );
}

#[test]
fn maestro_reports_missing_dependencies_once_per_distinct_message() {
    // The reference prints the same missing-b,c line twice; Maestro prints it once.
    let schema = json!({"dependentRequired":{"a":["b","c"]}});
    let arguments = json!({"a":1});
    assert_eq!(
        check_object(schema.clone(), arguments.as_object().unwrap().clone()).unwrap_err(),
        "Validation failed for tool \"check\":\n  - root: must have properties b, c when property a is present\n\nReceived arguments:\n{\n  \"a\": 1\n}"
    );
    assert_eq!(
        check_object(
            json!({"allOf":[schema.clone(),schema]}),
            arguments.as_object().unwrap().clone()
        )
        .unwrap_err(),
        "Validation failed for tool \"check\":\n  - root: must have properties b, c when property a is present\n\nReceived arguments:\n{\n  \"a\": 1\n}"
    );
}

#[test]
fn maestro_resolves_relative_references_against_the_enclosing_id() {
    for keyword in ["$ref", "$dynamicRef"] {
        let schema = json!({
            "$id":"https://e.example/root/",
            "$defs":{
                "number":{"$id":"target","type":"number"},
                "string":{"$id":"sub/target","type":"string"}
            },
            "properties":{"n":{"$id":"sub/", keyword:"target"}}
        });
        // The reference accepts n:1 and rejects n:"x" by using the document root.
        assert_eq!(
            check_object(schema.clone(), json!({"n":1}).as_object().unwrap().clone()).unwrap_err(),
            "Validation failed for tool \"check\":\n  - n: must be string\n\nReceived arguments:\n{\n  \"n\": 1\n}"
        );
        assert_eq!(
            check_object(schema, json!({"n":"x"}).as_object().unwrap().clone()).unwrap(),
            json!({"n":"x"}).as_object().unwrap().clone()
        );
    }
}

const ROOT: &str = "https://e.example/root/";
const KEYWORDS: [&str; 3] = ["$ref", "$recursiveRef", "$dynamicRef"];
const MUST_BE_STRING: &str = "  - n: must be string";
const MUST_BE_NUMBER: &str = "  - n: must be number";
const SCHEMA_IS_FALSE: &str = "  - n: schema is false";

type Checked = Result<(), Box<dyn std::error::Error>>;
type Rows<'a> = Vec<(Value, Vec<&'a str>)>;

/// Check one complete public result; no diagnostic lines means unchanged acceptance.
fn expect(schema: &Value, arguments: Value, lines: &[&str]) -> Checked {
    let arguments: maestro_models::JsonObject = serde_json::from_value(arguments)?;
    let expected = if lines.is_empty() {
        Ok(arguments.clone())
    } else {
        Err(format!(
            "Validation failed for tool \"check\":\n{}\n\nReceived arguments:\n{}",
            lines.join("\n"),
            serde_json::to_string_pretty(&arguments)?
        ))
    };
    assert_eq!(
        check_object(schema.clone(), arguments),
        expected,
        "{schema}"
    );
    Ok(())
}

/// Check each `(schema, rows)` pair with the schema applied to property `n`.
fn expect_at_n(cases: Vec<(Value, Rows)>) -> Checked {
    for (schema, rows) in cases {
        let schema = json!({"properties":{"n":schema}});
        for (n, lines) in rows {
            expect(&schema, json!({"n":n}), &lines)?;
        }
    }
    Ok(())
}

/// Leaf pair S: `"x"` is accepted and `1` is not a string.
fn expect_string_leaf(schema: &Value) -> Checked {
    expect(schema, json!({"n":"x"}), &[])?;
    expect(schema, json!({"n":1}), &[MUST_BE_STRING])
}

/// Leaf pair N: `1` is accepted and `"x"` is not a number.
fn expect_number_leaf(schema: &Value) -> Checked {
    expect(schema, json!({"n":"x"}), &[MUST_BE_NUMBER])?;
    expect(schema, json!({"n":1}), &[])
}

/// Leaf pair F: no input reaches an assertion.
fn expect_unreachable(schema: &Value) -> Checked {
    expect(schema, json!({"n":"x"}), &[SCHEMA_IS_FALSE])?;
    expect(schema, json!({"n":1}), &[SCHEMA_IS_FALSE])
}

/// Every keyword word of the given length.
fn keyword_words(length: usize) -> Vec<Vec<&'static str>> {
    (0..length).fold(vec![vec![]], |words, _| {
        words
            .into_iter()
            .flat_map(|word| KEYWORDS.map(|keyword| [word.clone(), vec![keyword]].concat()))
            .collect()
    })
}

/// One generated reference chain: nodes nest by `parents` (`None` is the document root)
/// and are named by pointer (`P`), fragment (`F`), relative (`L`), directory (`D`)
/// or absolute (`U`) identifiers.
struct Chain {
    direction: &'static str,
    parents: Vec<Option<usize>>,
    form: char,
    keywords: Vec<&'static str>,
}

impl Chain {
    fn id(&self, node: usize) -> Option<String> {
        let name = node + 1;
        match self.form {
            'F' => Some(format!("#r{name}")),
            'L' => Some(format!("r{name}")),
            'D' => Some(format!("r{name}/")),
            'U' => Some(format!("{ROOT}r{name}")),
            _ => None,
        }
    }

    fn uri(&self, node: Option<usize>) -> Result<url::Url, Box<dyn std::error::Error>> {
        let Some(node) = node else {
            return Ok(ROOT.parse()?);
        };
        let parent = self.uri(self.parents[node])?;
        Ok(match self.id(node) {
            Some(id) => parent.join(&id)?,
            None => parent,
        })
    }

    fn pointer(&self, node: usize) -> String {
        let parent = self.parents[node].map_or_else(String::new, |parent| self.pointer(parent));
        format!("{parent}/$defs/r{}", node + 1)
    }

    fn reference(
        &self,
        from: Option<usize>,
        to: usize,
    ) -> Result<String, Box<dyn std::error::Error>> {
        Ok(match self.form {
            'P' => format!("#{}", self.pointer(to)),
            'F' => format!("{ROOT}#r{}", to + 1),
            'U' => self.uri(Some(to))?.to_string(),
            _ => self
                .uri(from)?
                .make_relative(&self.uri(Some(to))?)
                .ok_or("relative reference")?,
        })
    }

    fn node(&self, node: usize) -> Result<Value, Box<dyn std::error::Error>> {
        let mut schema = json!({});
        if let Some(id) = self.id(node) {
            schema["$id"] = json!(id);
        }
        if node + 1 < self.parents.len() {
            schema[self.keywords[node + 1]] = json!(self.reference(Some(node), node + 1)?);
        } else {
            schema["type"] = json!("string");
        }
        self.insert_children(&mut schema, Some(node))?;
        Ok(schema)
    }

    fn insert_children(&self, schema: &mut Value, parent: Option<usize>) -> Checked {
        for (node, _) in self
            .parents
            .iter()
            .enumerate()
            .filter(|(_, p)| **p == parent)
        {
            schema["$defs"][format!("r{}", node + 1)] = self.node(node)?;
        }
        Ok(())
    }

    fn schema(&self) -> Result<Value, Box<dyn std::error::Error>> {
        let mut schema = json!({
            "$id": ROOT,
            "properties": {"n": {self.keywords[0]: self.reference(None, 0)?}}
        });
        self.insert_children(&mut schema, None)?;
        Ok(schema)
    }

    /// A recursive reference cannot leave the resource that declares it.
    fn leaves_recursive_resource(&self) -> bool {
        self.form != 'P'
            && self
                .keywords
                .iter()
                .enumerate()
                .skip(1)
                .any(|(hop, keyword)| {
                    *keyword == "$recursiveRef" && self.direction.chars().nth(hop - 1) != Some('N')
                })
    }
}

/// Chains of one to three hops over sibling, nested and ancestor targets.
fn chains() -> Vec<Chain> {
    let shapes = [
        ("", "-"),
        ("S", "--"),
        ("N", "-0"),
        ("A", "1-"),
        ("SS", "---"),
        ("SN", "--1"),
        ("SA", "22-"),
        ("NS", "-00"),
        ("NN", "-01"),
        ("NA", "20-"),
        ("AS", "1--"),
        ("AN", "1-1"),
        ("AA", "12-"),
    ];
    let mut chains = Vec::new();
    for (direction, parents) in shapes {
        let parents: Vec<_> = parents
            .chars()
            .map(|node| {
                node.to_digit(10)
                    .and_then(|node| usize::try_from(node).ok())
            })
            .collect();
        for form in ['P', 'F', 'L', 'D', 'U'] {
            for keywords in keyword_words(parents.len()) {
                let parents = parents.clone();
                chains.push(Chain {
                    direction,
                    parents,
                    form,
                    keywords,
                });
            }
        }
    }
    assert_eq!(chains.len(), 1365);
    chains
}

/// Check the chains of the given forms that do (or do not) leave a recursive resource.
fn expect_chains(forms: &[char], leaves: bool) -> Checked {
    let selected: Vec<_> = chains()
        .into_iter()
        .filter(|chain| forms.contains(&chain.form) && chain.leaves_recursive_resource() == leaves)
        .collect();
    assert!(!selected.is_empty());
    for chain in selected {
        if leaves {
            expect_unreachable(&chain.schema()?)?;
        } else {
            expect_string_leaf(&chain.schema()?)?;
        }
    }
    Ok(())
}

#[test]
fn maestro_shares_one_resource_for_pointer_chains_of_every_keyword() {
    expect_chains(&['P'], false).unwrap();
}

#[test]
fn maestro_follows_fragment_relative_and_absolute_ids_across_resources() {
    expect_chains(&['F', 'L', 'U'], false).unwrap();
}

#[test]
fn maestro_confines_recursive_references_to_their_active_resource() {
    expect_chains(&['F', 'L', 'U'], true).unwrap();
}

#[test]
fn maestro_resolves_directory_ids_against_each_lexical_parent() {
    expect_chains(&['D'], false).unwrap();
}

#[test]
fn maestro_confines_recursive_search_for_directory_ids() {
    expect_chains(&['D'], true).unwrap();
}

#[test]
fn maestro_resolves_every_reference_kind_without_a_root_id() {
    for keyword in KEYWORDS {
        for (target, reference) in [
            (json!({"type":"string"}), "#/$defs/child"),
            (json!({"$id":"child","type":"string"}), "child"),
            (
                json!({"$id":"https://e.example/child","type":"string"}),
                "https://e.example/child",
            ),
        ] {
            let schema = json!({
                "$defs":{"child":target},
                "properties":{"n":{keyword:reference}}
            });
            expect_string_leaf(&schema).unwrap();
        }
    }
}

#[test]
fn maestro_resolves_each_hop_against_its_target_resource() {
    for outer in KEYWORDS {
        for inner in KEYWORDS {
            let schema = json!({
                "$id":ROOT,
                "$defs":{"container":{"$id":"sub/","$defs":{
                    "entry":{"$id":"entry",inner:"leaf"},
                    "leaf":{"$id":"leaf","type":"string"}
                }}},
                "properties":{"n":{outer:"sub/entry"}}
            });
            if inner == "$recursiveRef" {
                expect_unreachable(&schema).unwrap();
            } else {
                expect_string_leaf(&schema).unwrap();
            }
        }
    }
}

#[test]
fn maestro_keeps_the_inherited_uri_for_every_target_return_route() {
    for outer in KEYWORDS {
        for (marks, reference) in [
            (json!({"$id":"entry"}), format!("{ROOT}sub/entry")),
            (json!({}), format!("{ROOT}sub/#/$defs/entry")),
            (json!({}), "#/$defs/box/$defs/entry".to_owned()),
            (json!({"$anchor":"go"}), format!("{ROOT}sub/#go")),
            (json!({"$dynamicAnchor":"go"}), format!("{ROOT}sub/#go")),
        ] {
            let mut entry = marks;
            entry["$ref"] = json!("leaf");
            let schema = json!({
                "$id":ROOT,
                "$defs":{"box":{"$id":format!("{ROOT}sub/"),"$defs":{
                    "entry":entry,
                    "leaf":{"$id":"leaf","type":"string"}
                }}},
                "properties":{"n":{outer:reference}}
            });
            expect_string_leaf(&schema).unwrap();
        }
    }
}

#[test]
fn maestro_keeps_local_pointers_valid_across_mixed_identifier_hops() {
    for outer in KEYWORDS {
        for inner in KEYWORDS {
            let schema = json!({
                "$id":ROOT,
                "$defs":{"directory":{"$id":"sub/","$defs":{"entry":{
                    "$id":"a", inner:"#/$defs/next",
                    "$defs":{"next":{"$id":"#next","$ref":"#/$defs/end","$defs":{"end":{"type":"string"}}}}
                }}}},
                "properties":{"n":{outer:"sub/a"}}
            });
            expect_string_leaf(&schema).unwrap();
        }
    }
}

#[test]
fn maestro_does_not_treat_every_hash_bearing_id_as_an_alias() {
    for id in ["path#alias", "https://e.example/path#alias"] {
        let schema = json!({
            "$defs":{"target":{"$id":id,"type":"string"}},
            "properties":{"n":{"$ref":id}}
        });
        expect_unreachable(&schema).unwrap();
    }
}

#[test]
fn maestro_selects_the_identified_resource_for_an_empty_fragment() {
    for keyword in KEYWORDS {
        let target = json!({"$id":"leaf","type":"string"});
        let schema = json!({"$defs":{"target":target},"properties":{"n":{keyword:"leaf#"}}});
        expect_string_leaf(&schema).unwrap();
        for missing in ["missing#", "missing", "#missing"] {
            let schema = json!({"$defs":{"target":target},"properties":{"n":{keyword:missing}}});
            expect_unreachable(&schema).unwrap();
        }
    }
}

#[test]
fn maestro_overrides_dynamic_anchors_only_for_anchor_references() {
    for (target, reference, number) in [
        (
            json!({"$dynamicAnchor":"kind","type":"string"}),
            "#/$defs/str",
            false,
        ),
        (
            json!({"$dynamicAnchor":"kind","type":"string"}),
            "#%2F$defs%2Fstr",
            false,
        ),
        (
            json!({"$dynamicAnchor":"kind","$anchor":"str","type":"string"}),
            "#str",
            true,
        ),
        (json!({"$anchor":"kind","type":"string"}), "#kind", false),
    ] {
        let schema = json!({
            "$id":ROOT,
            "$defs":{"num":{"$dynamicAnchor":"kind","type":"number"},"str":target},
            "properties":{"n":{"$dynamicRef":reference}}
        });
        if number {
            expect_number_leaf(&schema).unwrap();
        } else {
            expect_string_leaf(&schema).unwrap();
        }
    }
}

/// A dynamic anchor declared in a resource that the reference chain enters but never evaluates.
fn hidden_dynamic_schema(route: &str, outer: &str, via: bool) -> Value {
    let entry = json!({"$dynamicRef":"https://e.example/leaf#kind"});
    let (name, node, reference) = match route {
        "anchor" => (
            "entry",
            json!({"$anchor":"entry","$dynamicRef":"https://e.example/leaf#kind"}),
            "https://e.example/box/#entry",
        ),
        "tuple" => (
            "tuple",
            json!([entry]),
            "https://e.example/box/#/$defs/tuple/0",
        ),
        _ => ("entry", entry, "https://e.example/box/#/$defs/entry"),
    };
    let mut schema = json!({
        "$id":ROOT,
        "$defs":{"box":{"$id":"https://e.example/box/","$defs":{
            "override":{"$dynamicAnchor":"kind","type":"number"},
            "leaf":{"$id":"https://e.example/leaf","$dynamicAnchor":"kind","type":"string"},
            name:node
        }}},
        "properties":{"n":{outer:reference}}
    });
    if via {
        schema["$defs"]["via"] = json!({"$id":"https://e.example/via","$ref":reference});
        schema["properties"]["n"] = json!({outer:"https://e.example/via"});
    }
    schema
}

#[test]
fn maestro_activates_the_target_resource_chain_for_dynamic_anchors() {
    for route in ["pointer", "anchor", "tuple"] {
        for outer in KEYWORDS {
            for via in [false, true] {
                expect_number_leaf(&hidden_dynamic_schema(route, outer, via)).unwrap();
            }
        }
    }
}

#[test]
fn maestro_finds_dynamic_anchors_declared_in_tuple_schemas() {
    let schema = json!({
        "$id":ROOT,
        "$defs":{"box":{
            "$id":"https://e.example/box/", "$ref":"#/$defs/entry",
            "properties":{"unused":{"prefixItems":[{"$dynamicAnchor":"kind","type":"number"}]}},
            "$defs":{
                "entry":{"$dynamicRef":"https://e.example/leaf#kind"},
                "leaf":{"$id":"https://e.example/leaf","$dynamicAnchor":"kind","type":"string"}
            }
        }},
        "properties":{"n":{"$ref":"https://e.example/box/"}}
    });
    expect_number_leaf(&schema).unwrap();
}

#[test]
fn maestro_isolates_dynamic_bindings_between_sibling_references() {
    let mut schema = hidden_dynamic_schema("pointer", "$ref", false);
    schema["properties"] = json!({
        "a":{"$ref":"https://e.example/box/#/$defs/entry"},
        "b":{"$dynamicRef":"https://e.example/leaf#kind"}
    });
    expect(&schema, json!({"a":1,"b":"x"}), &[]).unwrap();
    expect(&schema, json!({"a":1,"b":1}), &["  - b: must be string"]).unwrap();
    expect(
        &schema,
        json!({"a":"x","b":"x"}),
        &["  - a: must be number"],
    )
    .unwrap();
}

#[test]
fn maestro_binds_recursive_anchors_of_the_entered_resource() {
    for keyword in KEYWORDS {
        let schema = json!({
            "$id":ROOT,
            "$defs":{"box":{
                "$id":"https://e.example/box/", "$recursiveAnchor":true, "type":"number",
                "$defs":{"entry":{"$recursiveRef":"#"}}
            }},
            "properties":{"n":{keyword:"https://e.example/box/#/$defs/entry"}}
        });
        expect_number_leaf(&schema).unwrap();
    }
}

/// An outer and an inner resource, each tagging its own instance level.
fn tagged_resource(name: &str, next: &Value, extra: Value) -> Value {
    let mut resource = json!({
        "$id":format!("https://e.example/{name}"),
        "properties":{"tag":{"const":name},"next":next}
    });
    if let (Value::Object(resource), Value::Object(extra)) = (&mut resource, extra) {
        resource.extend(extra);
    }
    resource
}

#[test]
fn maestro_selects_outermost_recursive_and_dynamic_bindings_by_instance_depth() {
    let recursive = json!({"$recursiveRef":"#"});
    let dynamic = json!({"$dynamicRef":"#node"});
    for (outer_extra, inner_extra, next, accepted, rejected) in [
        (
            json!({"$recursiveAnchor":true}),
            json!({"$recursiveAnchor":true}),
            &recursive,
            "outer",
            "inner",
        ),
        (
            json!({"$recursiveAnchor":true}),
            json!({"$recursiveAnchor":false}),
            &recursive,
            "inner",
            "outer",
        ),
        (
            json!({"$dynamicAnchor":"node"}),
            json!({"$dynamicAnchor":"node"}),
            &dynamic,
            "outer",
            "inner",
        ),
        (
            json!({"$dynamicAnchor":"node"}),
            json!({"$dynamicAnchor":"other","$defs":{"local":{"$dynamicAnchor":"node"}}}),
            &dynamic,
            "outer",
            "local",
        ),
    ] {
        let outer = tagged_resource(
            "outer",
            &json!({"$ref":"https://e.example/inner"}),
            outer_extra,
        );
        let schema = json!({
            "$id":"https://e.example/",
            "$defs":{"outer":outer,"inner":tagged_resource("inner", next, inner_extra)},
            "properties":{"n":{"$ref":"https://e.example/outer"}}
        });
        let input =
            |tag: &str| json!({"n":{"tag":"outer","next":{"tag":"inner","next":{"tag":tag}}}});
        expect(&schema, input(accepted), &[]).unwrap();
        let constant = ["  - n.next.next.tag: must be equal to constant"];
        expect(&schema, input(rejected), &constant).unwrap();
    }
}

#[test]
fn maestro_rejects_reference_rings_of_every_keyword_mix() {
    for size in 1..=3 {
        for keywords in keyword_words(size) {
            let mut schema = json!({"properties":{"n":{keywords[0]:"#/$defs/s0"}}});
            for (index, keyword) in keywords.iter().enumerate() {
                let next = format!("#/$defs/s{}", (index + 1) % size);
                schema["$defs"][format!("s{index}")] = json!({*keyword: next});
            }
            expect_unreachable(&schema).unwrap();
        }
    }
}

const UP: &str = "  - n: must not have unevaluated properties";
const UI: &str = "  - n: must not have unevaluated items";
const NOT_VALID: &str = "  - n: must not be valid";
const ANY_OF: &str = "  - n: must match a schema in anyOf";
const ONE_OF: &str = "  - n: must match exactly one schema in oneOf";
const THEN: &str = "  - n: must match \"then\" schema";
const ELSE: &str = "  - n: must match \"else\" schema";
const CONTAINS: &str = "  - n: must contain at least 1 valid item";
const MIN_TWO_ITEMS: &str = "  - n: must not have fewer than 2 items";
const REQUIRED_A: &str = "  - n.a: must have required properties a";
const REQUIRED_B: &str = "  - n.b: must have required properties b";
const REQUIRED_X: &str = "  - n.x: must have required properties x";
const REQUIRED_Y: &str = "  - n.y: must have required properties y";

#[test]
fn maestro_exports_no_marks_from_negation() {
    expect_at_n(vec![
        (
            json!({"not":{"properties":{"a":true},"minProperties":2},"unevaluatedProperties":false}),
            vec![(json!({}), vec![]), (json!({"a":1}), vec![UP]), (json!({"a":1,"b":2}), vec![NOT_VALID, UP])],
        ),
        (
            json!({"not":{"items":[true],"minItems":2},"unevaluatedItems":false}),
            vec![(json!([]), vec![]), (json!([1]), vec![UI]), (json!([1, 2]), vec![NOT_VALID, UI])],
        ),
        (
            json!({"not":{"properties":{"a":true},"required":["a"]},"unevaluatedProperties":false}),
            vec![(json!({}), vec![]), (json!({"a":1}), vec![NOT_VALID, UP])],
        ),
        (
            json!({"not":{"prefixItems":[true],"minItems":1},"unevaluatedItems":false}),
            vec![(json!([]), vec![]), (json!([1]), vec![NOT_VALID, UI])],
        ),
        (
            json!({"properties":{"a":true},"not":{"properties":{"b":true},"minProperties":3},"unevaluatedProperties":false}),
            vec![(json!({"a":1}), vec![]), (json!({"a":1,"b":2}), vec![UP])],
        ),
        (
            json!({"prefixItems":[true],"not":{"items":[true,true],"minItems":3},"unevaluatedItems":false}),
            vec![(json!([1]), vec![]), (json!([1, 2]), vec![UI])],
        ),
    ])
    .unwrap();
}

#[test]
fn maestro_exports_conditional_marks_only_from_the_successful_branch() {
    expect_at_n(vec![
        (
            json!({"if":{"properties":{"a":true},"required":["a"]},"then":{"properties":{"b":true}},"else":{"properties":{"c":true}},"unevaluatedProperties":false}),
            vec![(json!({"a":1,"b":2}), vec![]), (json!({"c":2}), vec![]), (json!({"b":2}), vec![UP])],
        ),
        (
            json!({"if":{"properties":{"a":true},"minProperties":2},"else":true,"unevaluatedProperties":false}),
            vec![(json!({}), vec![]), (json!({"a":1}), vec![UP])],
        ),
        (
            json!({"if":{"items":[true],"minItems":2},"then":{"prefixItems":[true,true]},"else":true,"unevaluatedItems":false}),
            vec![(json!([]), vec![]), (json!([1]), vec![UI]), (json!([1, 2]), vec![])],
        ),
        (
            json!({"if":{"properties":{"a":true}},"unevaluatedProperties":false}),
            vec![(json!({"a":1}), vec![]), (json!({"b":2}), vec![UP])],
        ),
        (
            json!({"if":{"properties":{"a":true}},"then":false,"unevaluatedProperties":false}),
            vec![(json!({"a":1}), vec![THEN, UP])],
        ),
        (
            json!({"if":false,"else":{"properties":{"a":true},"required":["b"]},"unevaluatedProperties":false}),
            vec![(json!({"a":1}), vec![REQUIRED_B, ELSE, UP])],
        ),
    ])
    .unwrap();
}

#[test]
fn maestro_exports_marks_only_from_passing_combinator_branches() {
    expect_at_n(vec![
        (
            json!({"allOf":[{"properties":{"a":true}},{"required":["b"],"properties":{"b":true}}],"unevaluatedProperties":false}),
            vec![(json!({"a":1,"b":2}), vec![]), (json!({"a":1}), vec![REQUIRED_B, UP])],
        ),
        (
            json!({"allOf":[{"prefixItems":[true]},{"items":[true,true],"minItems":2}],"unevaluatedItems":false}),
            vec![(json!([1, 2]), vec![]), (json!([1]), vec![MIN_TWO_ITEMS, UI])],
        ),
        (
            json!({"anyOf":[{"properties":{"a":true}},{"required":["b"],"properties":{"b":true}}],"unevaluatedProperties":false}),
            vec![(json!({"a":1,"b":2}), vec![]), (json!({"a":1,"c":2}), vec![UP])],
        ),
        (
            json!({"anyOf":[{"items":[true,true],"minItems":3},{"prefixItems":[true]}],"unevaluatedItems":false}),
            vec![(json!([1]), vec![]), (json!([1, 2]), vec![UI])],
        ),
        (
            json!({"anyOf":[{"properties":{"a":true},"required":["x"]},{"properties":{"b":true},"required":["y"]}],"unevaluatedProperties":false}),
            vec![(json!({"a":1,"b":2}), vec![REQUIRED_X, REQUIRED_Y, ANY_OF, UP])],
        ),
        (
            json!({"oneOf":[{"properties":{"a":true},"required":["a"]},{"properties":{"b":true},"required":["b"]}],"unevaluatedProperties":false}),
            vec![
                (json!({"a":1}), vec![]),
                (json!({"a":1,"b":2}), vec![ONE_OF, UP]),
                (json!({}), vec![REQUIRED_A, REQUIRED_B, ONE_OF]),
            ],
        ),
        (
            json!({"oneOf":[{"items":[true],"maxItems":1},{"prefixItems":[true,true],"minItems":2}],"unevaluatedItems":false}),
            vec![(json!([1]), vec![]), (json!([1, 2]), vec![]), (json!([1, 2, 3]), vec![UI])],
        ),
        (
            json!({"dependentSchemas":{"a":{"properties":{"a":true,"b":true},"required":["b"]}},"unevaluatedProperties":false}),
            vec![(json!({}), vec![]), (json!({"a":1,"b":2}), vec![]), (json!({"a":1}), vec![REQUIRED_B, UP])],
        ),
    ])
    .unwrap();
}

#[test]
fn maestro_marks_object_members_only_when_they_succeed() {
    expect_at_n(vec![
        (
            json!({"properties":{"a":{"type":"number"}},"unevaluatedProperties":false}),
            vec![
                (json!({}), vec![]),
                (json!({"a":1}), vec![]),
                (json!({"a":"x"}), vec!["  - n.a: must be number", UP]),
                (json!({"a":1,"b":2}), vec![UP]),
            ],
        ),
        (
            json!({"patternProperties":{"^a":{"type":"number"},"a$":{"minimum":2}},"unevaluatedProperties":false}),
            vec![
                (json!({"a":2}), vec![]),
                (json!({"a":1}), vec!["  - n.a: must be >= 2"]),
                (json!({"a":"x"}), vec!["  - n.a: must be number"]),
                (json!({"b":2}), vec![UP]),
            ],
        ),
        (
            json!({"properties":{"a":true},"additionalProperties":{"type":"number"},"unevaluatedProperties":false}),
            vec![
                (json!({"a":1,"b":2}), vec![]),
                (json!({"a":1,"b":"x"}), vec!["  - n: must not have additional properties", UP]),
            ],
        ),
        (
            json!({"properties":{"a":{"properties":{"z":true},"unevaluatedProperties":false}},"unevaluatedProperties":false}),
            vec![
                (json!({"a":{"z":1}}), vec![]),
                (json!({"a":{"z":1},"z":2}), vec![UP]),
                (json!({"a":{"y":1}}), vec!["  - n.a: must not have unevaluated properties", UP]),
            ],
        ),
        (
            json!({"unevaluatedProperties":{"type":"number"}}),
            vec![(json!({"a":1}), vec![]), (json!({"a":"x"}), vec![UP])],
        ),
    ])
    .unwrap();
}

#[test]
fn maestro_marks_array_items_only_when_they_succeed() {
    expect_at_n(vec![
        (
            json!({"items":{"type":"number"},"unevaluatedItems":false}),
            vec![(json!([]), vec![]), (json!([1]), vec![]), (json!([1, "x"]), vec!["  - n.1: must be number", UI])],
        ),
        (
            json!({"prefixItems":[{"type":"number"}],"items":{"type":"string"},"unevaluatedItems":false}),
            vec![
                (json!([1, "x"]), vec![]),
                (json!(["x", 1]), vec!["  - n.1: must be string", "  - n.0: must be number", UI]),
            ],
        ),
        (
            json!({"items":[{"type":"number"}],"unevaluatedItems":false}),
            vec![(json!([1]), vec![]), (json!([1, 2]), vec![UI])],
        ),
        (
            json!({"items":[{"type":"number"}],"prefixItems":[{"minimum":2}],"unevaluatedItems":false}),
            vec![(json!([2]), vec![]), (json!([1]), vec!["  - n.0: must be >= 2"]), (json!([2, 3]), vec![UI])],
        ),
        (
            json!({"prefixItems":[{"prefixItems":[true],"unevaluatedItems":false}],"unevaluatedItems":false}),
            vec![(json!([[1]]), vec![]), (json!([[1], 2]), vec![UI])],
        ),
        (
            json!({"unevaluatedItems":{"type":"number"}}),
            vec![(json!([1]), vec![]), (json!(["x"]), vec![UI])],
        ),
    ])
    .unwrap();
}

#[test]
fn maestro_isolates_contains_item_marks_from_the_containing_array() {
    expect_at_n(vec![
        (
            json!({"contains":{"type":"number"},"unevaluatedItems":false}),
            vec![(json!([]), vec![CONTAINS]), (json!([1]), vec![UI]), (json!([1, "x"]), vec![UI])],
        ),
        (
            json!({"contains":{"type":"number"},"items":true,"unevaluatedItems":false}),
            vec![(json!([1]), vec![]), (json!(["x"]), vec![CONTAINS])],
        ),
        (
            json!({"contains":false,"minContains":0,"unevaluatedItems":false}),
            vec![(json!([]), vec![]), (json!([1]), vec![UI])],
        ),
        (
            json!({"contains":{"type":"number"},"items":true,"minContains":1,"maxContains":1,"unevaluatedItems":false}),
            vec![
                (json!([1]), vec![]),
                (json!([1, 2]), vec!["  - n: must contain at most 1 valid item"]),
                (json!(["x"]), vec![CONTAINS]),
            ],
        ),
        (
            json!({"contains":{"prefixItems":[true]},"unevaluatedItems":false}),
            vec![(json!([[1]]), vec![UI]), (json!([]), vec![CONTAINS])],
        ),
        (
            json!({"contains":{"prefixItems":[true]},"items":true,"unevaluatedItems":false}),
            vec![(json!([[1]]), vec![]), (json!([]), vec![CONTAINS])],
        ),
    ])
    .unwrap();
}

#[test]
fn maestro_carries_only_successful_reference_marks_for_every_keyword() {
    for keyword in KEYWORDS {
        for (unevaluated, members, rows) in [
            (
                "unevaluatedProperties",
                json!({"properties":{"a":true}}),
                vec![(json!({"a":1}), vec![]), (json!({"a":1,"b":2}), vec![UP])],
            ),
            (
                "unevaluatedItems",
                json!({"prefixItems":[true]}),
                vec![(json!([1]), vec![]), (json!([1, 2]), vec![UI])],
            ),
            (
                "unevaluatedProperties",
                json!({"properties":{"a":true},"minProperties":2}),
                vec![(
                    json!({"a":1}),
                    vec!["  - n: must not have fewer than 2 properties", UP],
                )],
            ),
            (
                "unevaluatedItems",
                json!({"items":[true],"minItems":2}),
                vec![(json!([1]), vec![MIN_TWO_ITEMS, UI])],
            ),
        ] {
            let schema = json!({
                "$id":ROOT, "$defs":{"members":members},
                "properties":{"n":{keyword:"#/$defs/members",unevaluated:false}}
            });
            for (n, lines) in rows {
                expect(&schema, json!({"n":n}), &lines).unwrap();
            }
        }
    }
}

/// Enter `child` from property `p` of `parent` through one route; reference routes store it
/// under `$defs` beside the reference.
fn enter(parent: &mut Value, route: &str, name: &str, reference: &str, child: Value) {
    parent["properties"]["p"] = match route {
        "properties" => child,
        "items" => json!({"items":child}),
        "allOf" => json!({"allOf":[child]}),
        _ => json!({route:reference,"$defs":{name:child}}),
    };
}

/// An unidentified outer schema with a recursive anchor, entered by `route`, that enters an inner
/// resource with its own anchor whose recursive reference sits one property below it.
fn recursive_chain(route: &str) -> Value {
    let mut outer = json!({"$recursiveAnchor":true,"properties":{"t":{"const":"outer"}}});
    let inner = json!({
        "$id":"https://e.example/inner", "$recursiveAnchor":true,
        "properties":{"t":{"const":"inner"},"q":{"$recursiveRef":"#"}}
    });
    enter(&mut outer, route, "inner", "https://e.example/inner", inner);
    let mut root = json!({"$id":"https://e.example/root"});
    let pointer = "https://e.example/root#/properties/p/$defs/outer";
    enter(&mut root, route, "outer", pointer, outer);
    json!({"properties":{"n":root}})
}

/// The recursive reference reaches the outer schema, not the inner resource that contains it.
fn expect_outer_recursive_binding(route: &str) -> Checked {
    let schema = recursive_chain(route);
    let hop = |inner: Value| json!({"p":if route == "items" { json!([inner]) } else { inner }});
    let arguments = |tag: &str| json!({"n":hop(hop(json!({"q":{"t":tag}})))});
    expect(&schema, arguments("outer"), &[])?;
    let path = if route == "items" {
        "n.p.0.p.0"
    } else {
        "n.p.p"
    };
    let line = format!("  - {path}.q.t: must be equal to constant");
    expect(&schema, arguments("inner"), &[&line])
}

#[test]
fn maestro_binds_the_recursive_anchor_of_a_child_entered_through_properties() {
    expect_outer_recursive_binding("properties").unwrap();
}

#[test]
fn maestro_binds_the_recursive_anchor_of_a_child_entered_through_items() {
    expect_outer_recursive_binding("items").unwrap();
}

#[test]
fn maestro_binds_the_recursive_anchor_of_a_child_entered_through_all_of() {
    expect_outer_recursive_binding("allOf").unwrap();
}

#[test]
fn maestro_binds_the_recursive_anchor_of_a_child_entered_through_ref() {
    expect_outer_recursive_binding("$ref").unwrap();
}

#[test]
fn maestro_binds_the_recursive_anchor_of_a_child_entered_through_dynamic_ref() {
    expect_outer_recursive_binding("$dynamicRef").unwrap();
}

#[test]
fn maestro_selects_an_entered_child_anchor_over_a_nested_resource_anchor() {
    let schema = json!({
        "$id":"https://e.example/root",
        "$defs":{"inner":{
            "$id":"https://e.example/inner", "$recursiveAnchor":true,
            "properties":{"next":{"$recursiveRef":"#"}}
        }},
        "properties":{"n":{
            "$recursiveAnchor":true,
            "properties":{"tag":{"const":"outer"},"next":{"$ref":"https://e.example/inner"}}
        }}
    });
    let arguments = |tag: &str| json!({"n":{"tag":"outer","next":{"next":{"tag":tag}}}});
    expect(&schema, arguments("outer"), &[]).unwrap();
    expect(
        &schema,
        arguments("wrong"),
        &["  - n.next.next.tag: must be equal to constant"],
    )
    .unwrap();
}

#[test]
fn maestro_prebinds_dynamic_anchors_only_for_schemas_with_an_identifier() {
    let target = |id: Option<&str>| {
        let mut schema = json!({
            "$dynamicRef":"#A",
            "$defs":{
                "first":{"$dynamicAnchor":"A","const":"a"},
                "second":{"$dynamicAnchor":"A","const":"b"}
            }
        });
        if let Some(id) = id {
            schema["$id"] = json!(id);
        }
        json!({"properties":{"n":schema}})
    };
    let rejected = ["  - n: must be equal to constant"];
    let plain = target(None);
    expect(&plain, json!({"n":"b"}), &[]).unwrap();
    expect(&plain, json!({"n":"a"}), &rejected).unwrap();
    let identified = target(Some("https://e.example/identified"));
    expect(&identified, json!({"n":"a"}), &[]).unwrap();
    expect(&identified, json!({"n":"b"}), &rejected).unwrap();
}

/// A schema that places a fragment as a function of that fragment.
type Placement = (&'static str, fn(Value) -> Value);

/// Schemas in which the checker builds a placed fragment.
const BUILT_PLACEMENTS: &[Placement] = &[
    ("root", |bad| json!(bad)),
    ("properties", |bad| json!({"properties":{"p":bad}})),
    (
        "patternProperties",
        |bad| json!({"patternProperties":{"^a":bad}}),
    ),
    (
        "additionalProperties",
        |bad| json!({"additionalProperties":bad}),
    ),
    ("propertyNames", |bad| json!({"propertyNames":bad})),
    (
        "unevaluatedProperties",
        |bad| json!({"unevaluatedProperties":bad}),
    ),
    (
        "dependentSchemas",
        |bad| json!({"dependentSchemas":{"p":bad}}),
    ),
    ("dependencies", |bad| json!({"dependencies":{"p":bad}})),
    (
        "dependencies beside a name list",
        |bad| json!({"dependencies":{"q":["r"],"p":bad}}),
    ),
    ("items", |bad| json!({"items":bad})),
    ("items tuple", |bad| json!({"items":[true,bad]})),
    ("prefixItems", |bad| json!({"prefixItems":[true,bad]})),
    (
        "additionalItems",
        |bad| json!({"items":[true],"additionalItems":bad}),
    ),
    ("contains", |bad| json!({"contains":bad})),
    (
        "contains with minContains 0",
        |bad| json!({"contains":bad,"minContains":0}),
    ),
    ("unevaluatedItems", |bad| json!({"unevaluatedItems":bad})),
    ("not", |bad| json!({"not":bad})),
    ("anyOf", |bad| json!({"anyOf":[bad]})),
    (
        "anyOf after a passing alternative",
        |bad| json!({"anyOf":[true,bad]}),
    ),
    ("oneOf", |bad| json!({"oneOf":[bad]})),
    (
        "oneOf after a passing alternative",
        |bad| json!({"oneOf":[true,bad]}),
    ),
    ("allOf", |bad| json!({"allOf":[bad]})),
    ("if", |bad| json!({"if":bad})),
    ("then", |bad| json!({"if":true,"then":bad})),
    ("else", |bad| json!({"if":true,"else":bad})),
    (
        "$ref to $defs",
        |bad| json!({"$ref":"#/$defs/d","$defs":{"d":bad}}),
    ),
    (
        "$ref to definitions",
        |bad| json!({"$ref":"#/definitions/d","definitions":{"d":bad}}),
    ),
    (
        "$ref chain",
        |bad| json!({"$ref":"#/$defs/a","$defs":{"a":{"$ref":"#/$defs/b"},"b":bad}}),
    ),
    (
        "$ref to an anchor",
        |bad| json!({"$ref":"#a","$defs":{"d":{"$anchor":"a","allOf":[bad]}}}),
    ),
    (
        "$ref to an identified resource",
        |bad| json!({"$ref":"https://schemas.example/d","$defs":{"d":{"$id":"https://schemas.example/d","allOf":[bad]}}}),
    ),
    (
        "$ref to the root",
        |bad| json!({"$ref":"#","properties":{"p":bad}}),
    ),
    (
        "$recursiveRef cycle",
        |bad| json!({"$recursiveAnchor":true,"properties":{"p":{"$recursiveRef":"#"}},"allOf":[bad]}),
    ),
    (
        "$dynamicRef",
        |bad| json!({"$id":"https://schemas.example/root","$dynamicRef":"#d","$defs":{"d":{"$dynamicAnchor":"d","allOf":[bad]}}}),
    ),
];

/// Schemas in which the checker never builds a placed fragment.
const UNBUILT_PLACEMENTS: &[Placement] = &[
    ("then without if", |bad| json!({"then":bad})),
    ("else without if", |bad| json!({"else":bad})),
    ("unreferenced $defs", |bad| json!({"$defs":{"d":bad}})),
    (
        "unreferenced definitions",
        |bad| json!({"definitions":{"d":bad}}),
    ),
    (
        "unreferenced $defs beside a referenced one",
        |bad| json!({"$ref":"#/$defs/used","$defs":{"used":true,"unused":bad}}),
    ),
    ("unknown keyword", |bad| json!({"unknown":bad})),
    (
        "const data",
        |bad| json!({"properties":{"p":{"const":bad}}}),
    ),
    (
        "enum data",
        |bad| json!({"properties":{"p":{"enum":[bad]}}}),
    ),
    (
        "default data",
        |bad| json!({"properties":{"p":{"default":bad}}}),
    ),
    (
        "properties with a non-schema member",
        |bad| json!({"properties":{"p":bad,"q":1}}),
    ),
    (
        "patternProperties with a non-schema member",
        |bad| json!({"patternProperties":{"^a":bad,"^b":1}}),
    ),
    (
        "dependentSchemas with a non-schema member",
        |bad| json!({"dependentSchemas":{"p":bad,"q":1}}),
    ),
    (
        "dependencies with a non-schema member",
        |bad| json!({"dependencies":{"p":bad,"q":1}}),
    ),
    (
        "items tuple with a non-schema member",
        |bad| json!({"items":[bad,1]}),
    ),
    (
        "prefixItems with a non-schema member",
        |bad| json!({"prefixItems":[bad,1]}),
    ),
    (
        "allOf with a non-schema member",
        |bad| json!({"allOf":[bad,1]}),
    ),
    (
        "anyOf with a non-schema member",
        |bad| json!({"anyOf":[bad,1]}),
    ),
    (
        "oneOf with a non-schema member",
        |bad| json!({"oneOf":[bad,1]}),
    ),
    (
        "additionalItems without a tuple",
        |bad| json!({"additionalItems":bad}),
    ),
    (
        "additionalItems beside homogeneous items",
        |bad| json!({"items":{},"additionalItems":bad}),
    ),
];

/// A malformed expression written as each keyword that builds one, with either boolean schema
/// behind a `patternProperties` key.
fn malformed(source: &str) -> [Value; 3] {
    [
        json!({"pattern":source}),
        json!({"patternProperties":{source:true}}),
        json!({"patternProperties":{source:false}}),
    ]
}

/// The native error of an expression the Unicode matcher rejects.
fn native(source: &str) -> String {
    regress::Regex::with_flags(source, "u")
        .err()
        .map_or_else(String::new, |error| error.to_string())
}

/// Assert the outcome of one placed schema for each argument object.
fn assert_placed(
    label: &str,
    schema: &Value,
    argument_sets: &[maestro_models::JsonObject],
    outcome: impl Fn(&maestro_models::JsonObject) -> Result<maestro_models::JsonObject, String>,
) {
    for arguments in argument_sets {
        assert_eq!(
            check_object(schema.clone(), arguments.clone()),
            outcome(arguments),
            "{label}: {schema} against {arguments:?}"
        );
    }
}

#[test]
fn maestro_rejects_malformed_patterns_in_every_schema_the_checker_builds() {
    let argument_sets = [
        maestro_models::JsonObject::new(),
        maestro_models::JsonObject::from_iter([
            ("p".into(), json!("text")),
            ("q".into(), json!(3)),
        ]),
    ];
    let native = native("[");
    for (placement, place) in BUILT_PLACEMENTS {
        for fragment in malformed("[") {
            assert_placed(placement, &place(fragment), &argument_sets, |_| {
                Err(native.clone())
            });
        }
    }
}

#[test]
fn maestro_ignores_malformed_patterns_in_schemas_the_checker_never_builds() {
    let empty = [maestro_models::JsonObject::new()];
    for (placement, place) in UNBUILT_PLACEMENTS {
        for fragment in malformed("[") {
            assert_placed(placement, &place(fragment), &empty, |arguments| {
                Ok(arguments.clone())
            });
        }
    }
    for (name, schema, arguments) in [
        (
            "pattern that is not a string",
            json!({"pattern":5}),
            json!({}),
        ),
        ("pattern list", json!({"pattern":["["]}), json!({})),
        ("property name", json!({"properties":{"[":true}}), json!({})),
        ("required name", json!({"required":["["]}), json!({"[":1})),
        (
            "dependentRequired name",
            json!({"dependentRequired":{"[":["["]}}),
            json!({}),
        ),
    ] {
        let arguments = [arguments.as_object().unwrap().clone()];
        assert_placed(name, &schema, &arguments, |arguments| Ok(arguments.clone()));
    }
}

#[test]
fn maestro_reports_a_malformed_pattern_before_any_argument_is_checked() {
    let schema = json!({
        "type":"object", "required":["name"],
        "properties":{"name":{"type":"string","pattern":"["}}
    });
    for arguments in [
        json!({}),
        json!({"name":"text"}),
        json!({"name":3}),
        json!({"other":true}),
    ] {
        assert_eq!(
            check_object(schema.clone(), arguments.as_object().unwrap().clone()),
            Err(native("[")),
            "{arguments}"
        );
    }
}

/// Schemas with two malformed expressions and the source the build reaches first.
const BUILD_ORDER: &[(&str, &str, &str)] = &[
    (
        "a nested schema before the pattern beside it",
        r#"{"properties":{"p":{"pattern":"["}},"pattern":"("}"#,
        "[",
    ),
    (
        "patternProperties before pattern",
        r#"{"pattern":"(","patternProperties":{"[":true}}"#,
        "[",
    ),
    (
        "properties before allOf",
        r#"{"allOf":[{"pattern":"["}],"properties":{"p":{"pattern":"("}}}"#,
        "(",
    ),
    (
        "a pattern key before its schema",
        r#"{"patternProperties":{"(":{"pattern":"["}}}"#,
        "(",
    ),
    (
        "pattern before a reference target",
        r##"{"pattern":"(","$ref":"#/$defs/d","$defs":{"d":{"pattern":"["}}}"##,
        "(",
    ),
    (
        "a reference target before if",
        r##"{"$ref":"#/$defs/d","$defs":{"d":{"pattern":"["}},"if":{"pattern":"("}}"##,
        "[",
    ),
    (
        "contains before items",
        r#"{"items":{"pattern":"["},"contains":{"pattern":"("}}"#,
        "(",
    ),
    (
        "contains after items when minContains is 0",
        r#"{"items":{"pattern":"["},"contains":{"pattern":"("},"minContains":0}"#,
        "[",
    ),
    (
        "the first patternProperties key",
        r#"{"patternProperties":{"[":true,"(":true}}"#,
        "[",
    ),
    (
        "dependencies before properties",
        r#"{"properties":{"p":{"pattern":"("}},"dependencies":{"q":{"pattern":"["}}}"#,
        "[",
    ),
    (
        "patternProperties keys before additionalProperties",
        r#"{"patternProperties":{"(":true},"additionalProperties":{"pattern":"["}}"#,
        "(",
    ),
    (
        "oneOf before unevaluatedItems",
        r#"{"unevaluatedItems":{"pattern":"("},"oneOf":[{"pattern":"["}]}"#,
        "[",
    ),
];

#[test]
fn maestro_reports_the_first_malformed_pattern_in_build_order() {
    assert_ne!(native("["), native("("));
    for (row, schema, first) in BUILD_ORDER {
        let schema: Value = serde_json::from_str(schema).unwrap();
        assert_eq!(
            check_object(schema, maestro_models::JsonObject::new()),
            Err(native(first)),
            "{row}"
        );
    }
}

#[test]
fn maestro_skips_conversion_alternatives_whose_expressions_do_not_compile() {
    for keyword in ["anyOf", "oneOf"] {
        for (source, converted) in [("^a", json!(5.0)), ("[", json!("5"))] {
            for mut alternative in malformed(source) {
                alternative["type"] = json!("number");
                let schema = json!({"type":"object","properties":{"p":{keyword:[alternative, 1]}}});
                let arguments = maestro_models::JsonObject::from_iter([("p".into(), json!("5"))]);
                assert_eq!(
                    check_object(schema, arguments).map(|arguments| arguments["p"].clone()),
                    Ok(converted.clone()),
                    "{keyword} {source}"
                );
            }
        }
    }
}

#[test]
fn maestro_ignores_keyword_maps_written_as_arrays() {
    for (row, schema, arguments) in [
        (
            "properties, malformed pattern",
            json!({"properties":[{"pattern":"["}]}),
            json!({}),
        ),
        (
            "patternProperties, malformed pattern",
            json!({"patternProperties":[{"pattern":"["}]}),
            json!({}),
        ),
        (
            "dependencies, malformed pattern",
            json!({"dependencies":[{"pattern":"["}]}),
            json!({}),
        ),
        (
            "dependentSchemas, malformed pattern",
            json!({"dependentSchemas":[{"pattern":"["}]}),
            json!({}),
        ),
        (
            "properties, violated type",
            json!({"properties":[{"type":"string"}]}),
            json!({"0":1}),
        ),
        (
            "patternProperties, violated type",
            json!({"patternProperties":[{"type":"string"}]}),
            json!({"0":1}),
        ),
        (
            "dependencies, missing name",
            json!({"dependencies":[["x"]]}),
            json!({"0":1}),
        ),
        (
            "dependentSchemas, missing name",
            json!({"dependentSchemas":[{"required":["x"]}]}),
            json!({"0":1}),
        ),
        (
            "dependentRequired, missing name",
            json!({"dependentRequired":[["x"]]}),
            json!({"0":1}),
        ),
    ] {
        let arguments = [serde_json::from_value(arguments).unwrap()];
        assert_placed(row, &schema, &arguments, |arguments| Ok(arguments.clone()));
    }
}

/// Two scopes reach one shared dynamic reference; only the first scope has a live binding, so the
/// later scope resolves it to the static anchor whose pattern is `static_pattern`.
fn shared_dynamic_reference(static_pattern: &str) -> Value {
    json!({
        "properties":{
            "a":{"$dynamicAnchor":"x","pattern":"^a","properties":{"inner":{"$ref":"#/$defs/shared"}}},
            "b":{"properties":{"inner":{"$ref":"#/$defs/shared"}}}
        },
        "$defs":{
            "shared":{"$dynamicRef":"#x"},
            "x":{"$dynamicAnchor":"x","pattern":static_pattern}
        }
    })
}

#[test]
fn maestro_prepares_a_shared_reference_target_in_every_scope_that_reaches_it() -> Checked {
    let schema = shared_dynamic_reference("^b");
    expect(&schema, json!({"a":{"inner":"a1"}}), &[])?;
    expect(&schema, json!({"b":{"inner":"b1"}}), &[])?;
    expect(
        &schema,
        json!({"a":{"inner":"b1"}}),
        &["  - a.inner: must match pattern \"^a\""],
    )?;
    expect(
        &schema,
        json!({"b":{"inner":"a1"}}),
        &["  - b.inner: must match pattern \"^b\""],
    )
}

#[test]
fn maestro_reports_a_malformed_pattern_that_only_a_later_scope_reaches() {
    assert_eq!(
        check_object(
            shared_dynamic_reference("["),
            maestro_models::JsonObject::new()
        ),
        Err(native("["))
    );
}
