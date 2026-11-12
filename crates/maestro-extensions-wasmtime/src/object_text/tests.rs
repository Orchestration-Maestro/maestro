//! The spliced text equals the text a JSON tree prints after the same insertion.
use super::with_property;
use serde_json::{Value, json};

/// The text the tree-building route prints for the same insertion.
fn through_a_tree(value: &Value, key: &str, property: &Value) -> String {
    let mut tree = value.clone();
    tree.as_object_mut()
        .unwrap()
        .insert(key.to_owned(), property.clone());
    serde_json::to_string(&tree).unwrap()
}

#[test]
fn spliced_text_equals_the_tree_text() {
    for value in [
        json!({}),
        json!({"a": 1}),
        json!({"z": [1.5, -0.0, null], "a": {"nested": "}\",{"}, "m": "é\n\u{1}"}),
    ] {
        let property = json!("tool_call");
        assert_eq!(
            with_property(&value, "type", &property).unwrap(),
            through_a_tree(&value, "type", &property)
        );
    }
    let flag = json!(false);
    assert_eq!(
        with_property(&json!({"content": []}), "isError", &flag).unwrap(),
        through_a_tree(&json!({"content": []}), "isError", &flag)
    );
}

#[test]
fn a_value_that_is_not_an_object_is_refused() {
    for value in [json!([1]), json!("{}"), json!(null), json!(3)] {
        assert_eq!(
            with_property(&value, "type", &"x").unwrap_err(),
            "event is not an object"
        );
    }
}
