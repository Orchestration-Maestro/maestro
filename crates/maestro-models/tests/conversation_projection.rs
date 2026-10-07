mod support;
use maestro_models::*;
use serde_json::json;

fn assistant(blocks: Vec<AssistantContent>) -> AssistantMessage {
    AssistantMessage {
        provider: model().provider,
        api: model().api,
        model: model().id,
        timestamp: 11.0,
        content: blocks,
        usage: usage(),
        stop_reason: StopReason::ToolUse,
        error_message: None,
        diagnostics: None,
        response_model: Some("different-actual".into()),
        response_id: Some("response".into()),
    }
}
fn model() -> Model {
    Model {
        id: "text".into(),
        name: "Synthetic".into(),
        api: "synthetic".into(),
        provider: "test".into(),
        base_url: "synthetic:".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec!["text".into()],
        cost: TokenRates {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 0.0,
        max_tokens: 0.0,
        headers: None,
        compat: None,
    }
}
fn usage() -> Usage {
    Usage {
        input: 11.0,
        output: 7.0,
        cache_read: 3.0,
        cache_write: 2.0,
        total_tokens: 23.0,
        cost: UsageCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
            total: 0.0,
        },
    }
}
#[test]
fn whole_messages_round_trip_by_role() {
    let messages = [
        Message::User(UserMessage {
            content: UserContent::Text("input".into()),
            timestamp: 1.0,
        }),
        Message::Assistant(assistant(vec![AssistantContent::Text(TextContent {
            text: "reply".into(),
            text_signature: None,
        })])),
        Message::ToolResult(ToolResultMessage {
            tool_call_id: "call".into(),
            tool_name: "lookup".into(),
            content: vec![],
            details: Some(json!({"retained":true})),
            is_error: true,
            timestamp: 2.0,
        }),
    ];
    for message in messages {
        let wire = serde_json::to_value(&message).unwrap();
        let decoded: Message = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(decoded, message, "role {}", wire["role"]);
        assert_eq!(serde_json::to_value(decoded).unwrap(), wire);
    }
}
