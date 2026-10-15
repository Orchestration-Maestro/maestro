//! Exact selection-rule and shared-window recordings.
use super::*;
use serde::Deserialize;
use serde_json::{Value, json};
/// One pure rule invocation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Source operation.
    #[serde(rename = "fn")]
    operation: String,
    /// Complete ordered operands.
    args: Vec<Value>,
    /// Exact result with null/empty distinction.
    expected: Value,
}
/// Ordered optional string list with no defaulting.
fn enabled(value: &Value) -> Option<Vec<String>> {
    serde_json::from_value(value.clone()).unwrap()
}
/// Ordered nonoptional string list.
fn ids(value: &Value) -> Vec<String> {
    serde_json::from_value(value.clone()).unwrap()
}
/// Replays every recorded operation against its private domain owner.
#[test]
fn selection_rules_match_pinned_cases() {
    let cases: Vec<Case> = serde_json::from_str(include_str!(
        "../../../tests/fixtures/scoped_model_rules.json"
    ))
    .unwrap();
    assert_eq!(cases.len(), 296);
    for (index, case) in cases.into_iter().enumerate() {
        let a = &case.args;
        let result = match case.operation.as_str() {
            "isEnabled" => json!(is_enabled(
                enabled(&a[0]).as_deref(),
                a[1].as_str().unwrap()
            )),
            "toggle" => json!(toggle(enabled(&a[0]).as_deref(), a[1].as_str().unwrap())),
            "enableAll" => json!(enable_all(
                enabled(&a[0]).as_deref(),
                &ids(&a[1]),
                a.get(2).map(ids).as_deref()
            )),
            "clearAll" => json!(clear_all(
                enabled(&a[0]).as_deref(),
                &ids(&a[1]),
                a.get(2).map(ids).as_deref()
            )),
            "move" => json!(r#move(
                enabled(&a[0]).as_deref(),
                a[1].as_str().unwrap(),
                isize::try_from(a[2].as_i64().unwrap()).unwrap()
            )),
            "getSortedIds" => json!(get_sorted_ids(enabled(&a[0]).as_deref(), &ids(&a[1]))),
            "list_window" => {
                let range = list_window(
                    serde_json::from_value(a[0].clone()).unwrap(),
                    serde_json::from_value(a[1].clone()).unwrap(),
                    serde_json::from_value(a[2].clone()).unwrap(),
                );
                json!([range.start, range.end])
            }
            _ => panic!("unknown rule"),
        };
        assert_eq!(result, case.expected, "rule case {index}");
    }
}
