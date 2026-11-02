//! Arbitrary JSON fields remain semantic values in every guest specialization.
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

/// Verifies object details without converting them to JSON string values.
fn roundtrip<T: Serialize + DeserializeOwned>(value: &Value) {
    let decoded: T = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(serde_json::to_value(decoded).unwrap(), *value);
}

#[test]
fn arbitrary_payload_aliases_preserve_semantic_objects() {
    use crate::*;
    let data = json!({"z": [false, null, {"n": 2.5}], "a": 9_007_199_254_740_993_u64});
    roundtrip::<CompactionResult>(
        &json!({"summary":"s", "firstKeptEntryId":"e", "tokensBefore":2.5, "details":data}),
    );
    roundtrip::<CustomMessageInput>(
        &json!({"customType":"c", "content":"t", "display":false, "details":data}),
    );
    roundtrip::<BeforeAgentStartEventResult>(
        &json!({"message":{"customType":"c", "content":"t", "display":false, "details":data}}),
    );
    roundtrip::<CustomMessage>(
        &json!({"role":"custom", "customType":"c", "content":"t", "display":false, "details":data, "timestamp":2.5}),
    );
    roundtrip::<BeforeProviderRequestEvent>(&json!({"payload":data}));
    roundtrip::<BeforeProviderRequestEventResult>(&data);
    roundtrip::<SessionBeforeTreeSummary>(&json!({"summary":"s", "details":data}));
    roundtrip::<SessionBeforeTreeResult>(&json!({"summary":{"summary":"s", "details":data}}));
    roundtrip::<ToolResultEventResult>(&json!({"details":data}));
    roundtrip::<AgentToolResult>(&json!({"content":[], "details":data}));
    roundtrip::<ToolInfo>(
        &json!({"name":"n", "description":"d", "parameters":data, "sourceInfo":{"path":"p","source":"s","scope":"project","origin":"top-level"}}),
    );
    roundtrip::<ToolMetadata>(
        &json!({"name":"n", "label":"l", "description":"d", "parameters":data}),
    );
    roundtrip::<ToolExecutionStartEvent>(&json!({"toolCallId":"i", "toolName":"t", "args":data}));
    roundtrip::<ToolExecutionUpdateEvent>(
        &json!({"toolCallId":"i", "toolName":"t", "args":data, "partialResult":data}),
    );
    roundtrip::<ToolExecutionEndEvent>(
        &json!({"toolCallId":"i", "toolName":"t", "result":data, "isError":false}),
    );
    roundtrip::<CustomToolCallEvent>(&json!({"toolCallId":"i", "toolName":"t", "input":data}));
    result_inputs(data);
}

/// Built-in detail schemas stay typed while each event's raw input remains arbitrary JSON.
fn result_inputs(data: Value) {
    use crate::*;
    let base =
        json!({"toolCallId":"i", "toolName":"t", "input":data, "content":[], "isError":false});
    roundtrip::<ToolResultEvent>(&base);
    roundtrip::<BashToolResultEvent>(&base);
    roundtrip::<ReadToolResultEvent>(&base);
    roundtrip::<EditToolResultEvent>(&base);
    roundtrip::<WriteToolResultEvent>(&base);
    roundtrip::<GrepToolResultEvent>(&base);
    roundtrip::<FindToolResultEvent>(&base);
    roundtrip::<LsToolResultEvent>(&base);
    let mut custom = base;
    custom["details"] = data;
    roundtrip::<CustomToolResultEvent>(&custom);
}
