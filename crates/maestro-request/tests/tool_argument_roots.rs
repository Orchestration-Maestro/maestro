//! Tool calls carry their complete JSON argument value through the shared records.
use maestro_request::types::{AssistantContent, AssistantMessage, StopReason, ToolCall, Usage};
use serde_json::{Value, json};

/// Wrap one call in the assistant message the records travel in.
fn assistant(call: ToolCall) -> AssistantMessage {
    AssistantMessage {
        content: vec![AssistantContent::ToolCall(call)],
        api: "custom".into(),
        provider: "provider".into(),
        model: "model".into(),
        response_model: None,
        response_id: None,
        diagnostics: None,
        usage: Usage::default(),
        stop_reason: StopReason::ToolUse,
        error_message: None,
        timestamp: 0.0,
    }
}

/// Every root kind a provider can deliver, with the signed zero among the numbers.
fn roots() -> Vec<Value> {
    vec![
        json!("text"),
        json!(""),
        json!(0),
        json!(-0.0),
        json!(42),
        json!(false),
        json!(true),
        Value::Null,
        json!([]),
        json!([1, 1, {"nested": [null, false]}]),
        json!({"b": 2, "10": 10, "2": 2, "a": [1, 1]}),
        json!({}),
    ]
}

#[test]
fn tool_call_records_roundtrip_each_root() {
    for arguments in roots() {
        let call = ToolCall {
            id: "call-1".into(),
            name: "tool".into(),
            arguments: arguments.clone(),
            thought_signature: Some("signature".into()),
        };
        let encoded = serde_json::to_value(assistant(call.clone())).expect("message serializes");
        assert_eq!(encoded["content"][0]["type"], json!("toolCall"));
        assert_eq!(encoded["content"][0]["id"], json!("call-1"));
        assert_eq!(encoded["content"][0]["name"], json!("tool"));
        assert_eq!(
            encoded["content"][0]["thoughtSignature"],
            json!("signature")
        );
        assert_eq!(encoded["content"][0]["arguments"], arguments);
        let decoded: AssistantMessage =
            serde_json::from_value(encoded).expect("message round-trips");
        let AssistantContent::ToolCall(decoded) = &decoded.content[0] else {
            panic!("expected a tool call, found {:?}", decoded.content[0]);
        };
        assert_eq!(decoded, &call);
        assert_signed_zero(&decoded.arguments, &arguments);
    }
}

/// A recorded negative zero keeps its sign through the record writer and reader.
fn assert_signed_zero(decoded: &Value, supplied: &Value) {
    let (Some(decoded), Some(supplied)) = (decoded.as_f64(), supplied.as_f64()) else {
        return;
    };
    assert_eq!(decoded == 0.0, supplied == 0.0);
    if supplied == 0.0 {
        assert_eq!(decoded.is_sign_negative(), supplied.is_sign_negative());
    }
}

#[test]
fn root_records_require_arguments_but_accept_null() {
    let explicit: ToolCall = serde_json::from_value(
        json!({"type": "toolCall", "id": "c", "name": "t", "arguments": null}),
    )
    .expect("an explicit null root is admissible");
    assert_eq!(explicit.arguments, Value::Null);
    assert_eq!(explicit.thought_signature, None);

    let unknown: ToolCall = serde_json::from_value(json!({
        "type": "toolCall",
        "id": "c",
        "name": "t",
        "arguments": [1],
        "unknown": true
    }))
    .expect("unknown members stay ignored");
    assert_eq!(unknown.arguments, json!([1]));

    let missing =
        serde_json::from_value::<ToolCall>(json!({"type": "toolCall", "id": "c", "name": "t"}));
    assert!(
        missing.is_err(),
        "a call without its argument field stays an error"
    );

    let encoded = serde_json::to_value(explicit).expect("the record serializes");
    assert_eq!(encoded["type"], json!("toolCall"));
}
