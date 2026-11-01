//! Truth, key order, canonical text and object admission against recorded results.
#![cfg(test)]
use maestro_json::object::{self, record_with};
use maestro_json::{
    BorrowedJson, array_index, canonical_entries, compact_json, compact_members,
    compact_members_with_default, compact_object, is_raw_truthy, is_truthy, pretty_object,
};
use serde::Deserialize;
use serde_json::value::RawValue;
use serde_json::{Map, Value, json};

/// A recorded truth decision; a missing input is an absent value.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Truth {
    input: Option<String>,
    expected: bool,
}

/// A recorded index decision.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Index {
    input: String,
    expected: Option<u32>,
}

/// A recorded key order.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Keys {
    input: Vec<String>,
    expected: Vec<String>,
}

/// A recorded text result.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Text {
    input: String,
    compact: String,
    pretty: Option<String>,
}

/// A recorded nonfinite spelling.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Nonfinite {
    input: String,
    expected: String,
}

/// Every recorded group.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    truth: Vec<Truth>,
    index: Vec<Index>,
    keys: Vec<Keys>,
    text: Vec<Text>,
    nonfinite: Vec<Nonfinite>,
}

/// Read the recorded groups.
fn fixture() -> Fixture {
    serde_json::from_str(include_str!("fixtures/boundary_rules.json")).unwrap()
}

#[test]
fn truth_distinguishes_absence_scalars_and_containers() {
    for case in fixture().truth {
        let raw = case
            .input
            .as_deref()
            .map(|text| serde_json::from_str::<&RawValue>(text).unwrap());
        assert_eq!(
            raw.is_some_and(is_raw_truthy),
            case.expected,
            "raw {:?}",
            case.input
        );
        let represented = case
            .input
            .as_deref()
            .and_then(|text| serde_json::from_str::<Value>(text).ok());
        if let Some(value) = represented {
            assert_eq!(is_truthy(&value), case.expected, "value {:?}", case.input);
        } else {
            assert!(case.input.is_none_or(|text| text.contains("e400")));
        }
    }
}

#[test]
fn array_indexes_reject_noncanonical_spellings() {
    for case in fixture().index {
        assert_eq!(array_index(&case.input), case.expected, "{:?}", case.input);
    }
}

#[test]
fn canonical_entries_keep_numeric_and_authored_order() {
    for case in fixture().keys {
        let values: Vec<usize> = (0..case.input.len()).collect();
        let entries = canonical_entries(case.input.iter().zip(&values));
        let keys: Vec<&str> = entries.iter().map(|(key, _)| key.as_str()).collect();
        assert_eq!(keys, case.expected);
        for (key, value) in entries {
            let position = case.input.iter().position(|k| k == key).unwrap();
            assert_eq!(
                std::ptr::from_ref(value),
                std::ptr::from_ref(&values[position])
            );
        }
    }
    let repeated = canonical_entries([("b", 1), ("2", 2), ("b", 3), ("2", 4), ("a", 5)]);
    assert_eq!(repeated, [("2", 2), ("2", 4), ("b", 1), ("b", 3), ("a", 5)]);
}

#[test]
fn text_writers_share_canonical_spelling() {
    for case in fixture().text {
        let value: Value = serde_json::from_str(&case.input).unwrap();
        assert_eq!(
            compact_json(&value).unwrap(),
            case.compact,
            "{}",
            case.input
        );
        let Value::Object(members) = &value else {
            assert!(case.pretty.is_none());
            continue;
        };
        assert_eq!(compact_object(members).unwrap(), case.compact);
        assert_eq!(Some(pretty_object(members).unwrap()), case.pretty);
    }
}

#[test]
fn nonfinite_values_use_null_without_changing_ordinary_serde() {
    for case in fixture().nonfinite {
        let number = match case.input.as_str() {
            "NaN" => f64::NAN,
            "Infinity" => f64::INFINITY,
            _ => f64::NEG_INFINITY,
        };
        let value = json!([number, { "n": number }]);
        assert_eq!(
            compact_json(&value).unwrap(),
            case.expected,
            "{}",
            case.input
        );
    }
    let negative_zero = Value::from(-0.0);
    assert_eq!(serde_json::to_string(&negative_zero).unwrap(), "-0.0");
    assert_eq!(compact_json(&negative_zero).unwrap(), "0");
}

#[test]
fn borrowed_members_keep_values_slices_and_key_order() {
    let nested = json!({ "z": 1, "10": [true], "2": { "b": -0.0, "1": 1e21 } });
    let items = [json!(9_007_199_254_740_993_u64), json!({ "k": [] })];
    let members = [
        ("z", BorrowedJson::Value(&nested)),
        ("10", BorrowedJson::Slice(&items)),
        ("2", BorrowedJson::Slice(&[])),
    ];
    let owned = Map::from_iter([
        ("z".to_owned(), nested.clone()),
        ("10".to_owned(), Value::Array(items.to_vec())),
        ("2".to_owned(), json!([])),
    ]);
    let expected = r#"{"2":[],"10":[9007199254740992,{"k":[]}],"z":{"2":{"1":1e+21,"b":0},"10":[true],"z":1}}"#;
    assert_eq!(compact_members(members.iter().copied()).unwrap(), expected);
    assert_eq!(compact_object(&owned).unwrap(), expected);
    assert_eq!(
        nested,
        json!({ "z": 1, "10": [true], "2": { "b": -0.0, "1": 1e21 } })
    );
    assert_eq!(compact_members(std::iter::empty()).unwrap(), "{}");
    let values = [
        json!("b first"),
        json!("1 first"),
        json!("b second"),
        json!("1 second"),
    ];
    let repeated = [
        ("b", BorrowedJson::Value(&values[0])),
        ("1", BorrowedJson::Value(&values[1])),
        ("b", BorrowedJson::Value(&values[2])),
        ("1", BorrowedJson::Value(&values[3])),
    ];
    assert_eq!(
        compact_members(repeated.iter().copied()).unwrap(),
        r#"{"1":"1 first","1":"1 second","b":"b first","b":"b second"}"#
    );
}

#[test]
fn default_member_projection_preserves_authored_values_and_canonical_order() {
    let fallback = json!("header");
    let items = [json!(-0.0), json!(1e21)];
    for (input, expected) in [
        (
            json!({"tail": true, "10": 10, "2": 2}),
            r#"{"2":2,"10":10,"type":"header","tail":true,"items":[0,1e+21]}"#,
        ),
        (
            json!({"tail": true, "type": "authored"}),
            r#"{"type":"authored","tail":true,"items":[0,1e+21]}"#,
        ),
        (
            json!({"tail": true, "type": null}),
            r#"{"type":null,"tail":true,"items":[0,1e+21]}"#,
        ),
    ] {
        let members = input
            .as_object()
            .unwrap()
            .iter()
            .map(|(name, value)| (name.as_str(), BorrowedJson::Value(value)))
            .chain(std::iter::once(("items", BorrowedJson::Slice(&items))));
        assert_eq!(
            compact_members_with_default(members, "type", BorrowedJson::Value(&fallback)).unwrap(),
            expected,
        );
    }
}

/// A record with one borrowed string field.
#[derive(Debug, Deserialize, PartialEq)]
struct Named<'a> {
    #[serde(borrow)]
    name: &'a str,
}

/// A record whose first duplicate would fail.
#[derive(Debug, Deserialize, PartialEq)]
struct Number {
    n: u8,
}

#[test]
fn object_readers_reject_positional_arrays() {
    let record: Named = object::from_str(r#"{"name":"x","extra":[1,{"deep":null}]}"#).unwrap();
    assert_eq!(record, Named { name: "x" });
    for text in [r#"["x"]"#, "null", "true", "1", r#""x""#] {
        assert!(object::from_str::<Named>(text).is_err(), "{text}");
    }
    assert!(serde_json::from_str::<Named>(r#"["x"]"#).is_ok());
    let nested = serde_json::from_str::<Vec<Number>>(r#"[{"n":1}]"#).unwrap();
    assert_eq!(nested, [Number { n: 1 }]);
    let wrapper = |text: &str| {
        let mut deserializer = serde_json::Deserializer::from_str(text);
        object::list::<_, Number>(&mut deserializer).map(|records| records.len())
    };
    assert_eq!(wrapper(r#"[{"n":1}]"#).unwrap(), 1);
    assert!(wrapper(r"[[1]]").is_err());
}

#[test]
fn object_readers_keep_typed_duplicate_policy() {
    assert!(object::from_str::<Number>(r#"{"n":1,"n":2}"#).is_err());
    assert!(object::from_str::<Number>(r#"{"n":1,"\u006e":2}"#).is_err());
    assert!(object::from_str::<Number>(r#"{"n":300,"n":2}"#).is_err());
    let materialized: Value = serde_json::from_str(r#"{"n":300,"n":2}"#).unwrap();
    assert_eq!(
        object::record::<_, Number>(materialized).unwrap(),
        Number { n: 2 }
    );
}

#[test]
fn object_lists_preserve_order_and_optional_null() {
    let read = |text: &str| {
        let mut deserializer = serde_json::Deserializer::from_str(text);
        object::list::<_, Number>(&mut deserializer)
            .map(|list| list.iter().map(|r| r.n).collect::<Vec<_>>())
    };
    assert_eq!(read(r#"[{"n":1},{"n":2},{"n":1}]"#).unwrap(), [1, 2, 1]);
    for text in ["null", "1", r"[1]", r"[null]", r"[[]]"] {
        assert!(read(text).is_err(), "{text}");
    }
    let optional = |text: &str| {
        let mut deserializer = serde_json::Deserializer::from_str(text);
        object::optional_list::<_, Number>(&mut deserializer)
    };
    assert_eq!(optional("null").unwrap(), None);
    assert_eq!(optional("[]").unwrap(), Some(Vec::new()));
    assert_eq!(
        optional(r#"[{"n":1},{"n":2},{"n":1}]"#).unwrap(),
        Some(vec![Number { n: 1 }, Number { n: 2 }, Number { n: 1 }])
    );
    assert!(optional(r#"[{"n":300}]"#).is_err());
    assert!(optional("[[]]").is_err());
}

#[test]
fn object_guard_uses_the_supplied_expectation() {
    let mut deserializer = serde_json::Deserializer::from_str("[1]");
    let error = record_with::<_, Number>(&mut deserializer, "an object").unwrap_err();
    assert!(error.to_string().contains("expected an object"), "{error}");
    let error = object::from_str::<Number>("[1]").unwrap_err();
    assert!(
        error.to_string().contains("expected a JSON object"),
        "{error}"
    );
    let missing = object::from_str::<Number>("{}").unwrap_err();
    assert!(
        missing.to_string().contains("missing field `n`"),
        "{missing}"
    );
    let duplicate = object::from_str::<Number>(r#"{"n":1,"n":2}"#).unwrap_err();
    assert!(
        duplicate.to_string().contains("duplicate field `n`"),
        "{duplicate}"
    );
}

#[test]
fn repl_json_array_format_reuses_canonical_owner() {
    let value = json!([{"z": ["é\n\"", -0.0, 1e21, 1e-7], "10": null, "2": true}]);
    assert_eq!(
        maestro_json::pretty_json(&value).unwrap(),
        "[\n  {\n    \"2\": true,\n    \"10\": null,\n    \"z\": [\n      \"é\\n\\\"\",\n      0,\n      1e+21,\n      1e-7\n    ]\n  }\n]"
    );
    assert_eq!(maestro_json::pretty_json(&Value::Null).unwrap(), "null");
}

#[test]
fn generic_pretty_json_preserves_canonical_numbers_and_keys() {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Query {
        input: String,
        expected: String,
    }
    let queries: Vec<Query> =
        serde_json::from_str(include_str!("fixtures/pretty_json.json")).unwrap();
    assert_eq!(queries.len(), 8);
    for query in queries {
        let value: Value = serde_json::from_str(&query.input).unwrap();
        assert_eq!(maestro_json::pretty_json(&value).unwrap(), query.expected);
    }
}

#[test]
fn truthy_display_borrows_strings_and_formats_other_values_once() {
    use maestro_json::display_truthy;
    use std::borrow::Cow;
    assert!(display_truthy(None).unwrap().is_none());
    for value in [json!(null), json!(false), json!(0), json!(-0.0), json!("")] {
        assert!(display_truthy(Some(&value)).unwrap().is_none());
    }
    for text in ["literal <&> 雪", " \t\u{feff}\u{85} "] {
        let value = json!(text);
        assert!(
            matches!(display_truthy(Some(&value)).unwrap(), Some(Cow::Borrowed(found)) if found == text)
        );
    }
    for (value, expected) in [
        (json!(true), "true"),
        (json!(1.25), "1.25"),
        (json!({}), "{}"),
        (json!([]), "[]"),
        (json!({"z":1,"a":2}), "{\"z\":1,\"a\":2}"),
        (json!(["b", "a", "b"]), "[\"b\",\"a\",\"b\"]"),
        (json!(1e21), "1e+21"),
    ] {
        assert!(
            matches!(display_truthy(Some(&value)).unwrap(), Some(Cow::Owned(found)) if found == expected)
        );
    }
}

/// Escaping the tilde first prevents a slash escape from being escaped again.
#[test]
fn pointer_tokens_keep_escape_order() {
    assert_eq!(maestro_json::escape_pointer_token("~/"), "~0~1");
    assert_eq!(maestro_json::escape_pointer_token("a/~1"), "a~1~01");
    assert_eq!(maestro_json::escape_pointer_token("plain"), "plain");
    assert_eq!(maestro_json::escape_pointer_token(""), "");
}
