//! Schema checking without invocation argument conversion.
use maestro_models::{
    Tool, ToolCall,
    arguments::validation::{validate_schema, validate_tool_arguments},
};
use serde_json::json;

#[test]
fn schema_check_does_not_coerce_values() {
    let schema =
        json!({"type":"object","properties":{"count":{"type":"number"}},"required":["count"]});
    let value = json!({"count":"3"});
    let original = (schema.clone(), value.clone());
    assert!(!validate_schema(&schema, &value).unwrap().is_empty());
    assert!(
        validate_schema(&schema, &json!({"count":3}))
            .unwrap()
            .is_empty()
    );
    assert_eq!((schema.clone(), value.clone()), original);
    assert!(validate_schema(&json!({"type":"string","pattern":"["}), &json!("x")).is_err());
    let tool = Tool {
        name: "counter".into(),
        description: String::new(),
        parameters: schema,
    };
    let call = ToolCall {
        id: "call".into(),
        name: "counter".into(),
        arguments: value.as_object().unwrap().clone(),
        thought_signature: None,
    };
    assert_eq!(
        validate_tool_arguments(&tool, &call).unwrap()["count"],
        json!(3.0)
    );
}
