//! The one-level object merge against settings-layer expectations.
#![cfg(test)]
use maestro_json::merge_one_level;
use serde_json::{Map, Value, json};

/// Reads a literal JSON object.
fn object(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        other => panic!("not an object: {other}"),
    }
}

#[test]
fn objects_merge_one_level_and_everything_else_replaces() {
    let merged = merge_one_level(
        object(json!({
            "nested": {"keep": 1, "swap": 1, "deep": {"x": 1}},
            "list": [1, 2],
            "scalar": 1,
            "object": {"x": 1}
        })),
        object(json!({
            "nested": {"swap": 2, "deep": {"y": 2}, "new": 3},
            "list": [3],
            "scalar": {"now": "object"},
            "object": null,
            "added": true
        })),
    );
    assert_eq!(
        Value::Object(merged),
        json!({
            "nested": {"keep": 1, "swap": 2, "deep": {"y": 2}, "new": 3},
            "list": [3],
            "scalar": {"now": "object"},
            "object": null,
            "added": true
        })
    );
}

#[test]
fn keys_keep_the_base_order_and_new_keys_follow() {
    let merged = merge_one_level(
        object(json!({"b": 1, "a": 1})),
        object(json!({"c": 1, "a": 2})),
    );
    assert_eq!(merged.keys().collect::<Vec<_>>(), ["b", "a", "c"]);
}
