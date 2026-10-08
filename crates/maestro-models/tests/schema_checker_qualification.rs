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
    validate_tool_arguments(&tool, &call).map_err(|error| error.message)
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
fn maestro_discards_failed_condition_property_and_item_marks() {
    let schema = json!({"if":{"required":["missing"],"properties":{"a":true}},"else":true,"unevaluatedProperties":false});
    let error = check_object(
        schema,
        maestro_models::JsonObject::from_iter([("a".into(), json!(1))]),
    )
    .unwrap_err();
    assert_eq!(
        error,
        "Validation failed for tool \"check\":\n  - root: must not have unevaluated properties\n\nReceived arguments:\n{\n  \"a\": 1\n}"
    );
    let schema =
        json!({"if":{"prefixItems":[true],"minItems":2},"else":true,"unevaluatedItems":false});
    assert!(
        check(schema, json!([1]))
            .unwrap_err()
            .contains("must not have unevaluated items")
    );
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
fn maestro_reports_native_errors_for_invalid_constraint_patterns() {
    let native = regress::Regex::with_flags("[", "u")
        .unwrap_err()
        .to_string();
    for schema in [
        json!({"pattern":"["}),
        json!({"patternProperties":{"[":false}}),
    ] {
        let input = if schema.get("pattern").is_some() {
            json!("text")
        } else {
            json!({"a":1})
        };
        let error = check(schema, input).unwrap_err();
        assert!(error.contains(&native), "{error}");
        assert!(!error.contains("must match pattern"), "{error}");
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
        assert!(check(json!({"pattern":pattern}), json!(invalid)).is_err());
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
fn maestro_isolates_nested_marks_and_carries_reference_marks() {
    for (schema, accepted, rejected, diagnostics) in [
        (
            json!({"properties":{"nested":{"properties":{"a":true},"unevaluatedProperties":false}},"unevaluatedProperties":false}),
            json!({"nested":{"a":1}}),
            json!({"nested":{"a":1},"a":1}),
            "  - value: must not have unevaluated properties",
        ),
        (
            json!({"properties":{"nested":{"properties":{"a":true},"unevaluatedProperties":false}},"unevaluatedProperties":false}),
            json!({"nested":{}}),
            json!({"nested":{"nested":1}}),
            "  - value.nested: must not have unevaluated properties\n  - value: must not have unevaluated properties",
        ),
        (
            json!({"$id":"https://marks.example/", "$defs":{"members":{"properties":{"a":true}}},"$ref":"#/$defs/members","unevaluatedProperties":false}),
            json!({"a":1}),
            json!({"a":1,"b":2}),
            "  - value: must not have unevaluated properties",
        ),
        (
            json!({"$id":"https://marks.example/", "$defs":{"members":{"prefixItems":[true]}},"$ref":"#/$defs/members","unevaluatedItems":false}),
            json!([1]),
            json!([1, 2]),
            "  - value: must not have unevaluated items",
        ),
    ] {
        assert_outcomes(schema, accepted, rejected, diagnostics).unwrap();
    }
}

#[test]
fn maestro_discards_failed_combinator_marks() {
    for (schema, accepted, rejected, diagnostics) in [
        (
            json!({"anyOf":[{"required":["missing"],"properties":{"a":true}},{"properties":{"b":true}}],"unevaluatedProperties":false}),
            json!({"b":1}),
            json!({"a":1,"b":1}),
            "  - value: must not have unevaluated properties",
        ),
        (
            json!({"oneOf":[{"minItems":2,"prefixItems":[true]},{"maxItems":2}],"unevaluatedItems":false}),
            json!([]),
            json!([1]),
            "  - value: must not have unevaluated items",
        ),
        (
            json!({"allOf":[{"required":["missing"],"properties":{"a":true,"missing":true}}],"unevaluatedProperties":false}),
            json!({"missing":1}),
            json!({"a":1}),
            "  - value.missing: must have required properties missing\n  - value: must not have unevaluated properties",
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
