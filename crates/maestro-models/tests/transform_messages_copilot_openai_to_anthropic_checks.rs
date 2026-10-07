use maestro_models::*;
use serde_json::json;

fn model() -> Model {
    serde_json::from_value(json!({"id":"claude-sonnet-4","name":"Claude Sonnet 4","api":"anthropic-messages","provider":"github-copilot","baseUrl":"https://api.individual.githubcopilot.com","reasoning":true,"input":["text","image"],"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0},"contextWindow":128000,"maxTokens":16000})).unwrap()
}
fn now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as f64
}
fn assistant(content: serde_json::Value) -> AssistantMessage {
    serde_json::from_value(json!({"role":"assistant","content":content,"api":"openai-responses","provider":"github-copilot","model":"gpt-5","usage":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}},"stopReason":"toolUse","timestamp":now()})).unwrap()
}
fn user(text: &str) -> Message {
    Message::User(UserMessage {
        content: UserContent::Text(text.into()),
        timestamp: now(),
    })
}
fn normalize(id: &str, _: &Model, _: &AssistantMessage) -> String {
    id.encode_utf16()
        .take(64)
        .map(|unit| {
            if unit <= 127 && ((unit as u8).is_ascii_alphanumeric() || unit == 45 || unit == 95) {
                unit as u8 as char
            } else {
                '_'
            }
        })
        .collect()
}

#[test]
fn copilot_foreign_thinking_becomes_readable_text() {
    let mut source = assistant(json!([
        {"type":"thinking","thinking":"Let me think about this...","thinkingSignature":"reasoning_content"},
        {"type":"text","text":"Hi there!"}
    ]));
    source.api = "openai-completions".into();
    source.model = "gpt-4o".into();
    source.stop_reason = StopReason::Stop;
    let messages = vec![user("hello"), Message::Assistant(source.clone())];
    let output = transform_messages(&messages, &model(), Some(&mut normalize));
    let Message::Assistant(actual) = &output[1] else {
        panic!()
    };
    assert_eq!(
        actual
            .content
            .iter()
            .filter(|b| matches!(b, AssistantContent::Thinking(_)))
            .count(),
        0
    );
    assert!(
        actual
            .content
            .iter()
            .filter(|b| matches!(b, AssistantContent::Text(_)))
            .count()
            >= 2
    );
    source.content = vec![
        AssistantContent::Text(TextContent {
            text: "Let me think about this...".into(),
            text_signature: None,
        }),
        AssistantContent::Text(TextContent {
            text: "Hi there!".into(),
            text_signature: None,
        }),
    ];
    assert_eq!(
        output,
        vec![messages[0].clone(), Message::Assistant(source)]
    );
}

#[test]
fn copilot_foreign_calls_lose_nonempty_thought_signature() {
    let signature = r#"{"type":"reasoning.encrypted","id":"call_123","data":"encrypted"}"#;
    let source = assistant(
        json!([{"type":"toolCall","id":"call_123","name":"bash","arguments":{"command":"ls"},"thoughtSignature":signature}]),
    );
    let real: Message = serde_json::from_value(json!({"role":"toolResult","toolCallId":"call_123","toolName":"bash","content":[{"type":"text","text":"output"}],"isError":false,"timestamp":now()})).unwrap();
    let messages = vec![
        user("run a command"),
        Message::Assistant(source.clone()),
        real,
    ];
    let output = transform_messages(&messages, &model(), Some(&mut normalize));
    let AssistantContent::ToolCall(original) = &source.content[0] else {
        panic!()
    };
    assert_eq!(
        original.read().unwrap().thought_signature.as_deref(),
        Some(signature)
    );
    let mut expected = serde_json::to_value(&messages).unwrap();
    expected[1]["content"][0]
        .as_object_mut()
        .unwrap()
        .remove("thoughtSignature");
    assert_eq!(serde_json::to_value(&output).unwrap(), expected);
    let Message::Assistant(actual) = &output[1] else {
        panic!()
    };
    let AssistantContent::ToolCall(call) = &actual.content[0] else {
        panic!()
    };
    assert_eq!(call.read().unwrap().thought_signature, None);
}

#[test]
fn copilot_trailing_call_receives_missing_result() {
    let messages = vec![
        user("read the file"),
        Message::Assistant(assistant(
            json!([{"type":"toolCall","id":"call_123|fc_123","name":"read","arguments":{"path":"README.md"}}]),
        )),
    ];
    let before = now();
    let output = transform_messages(&messages, &model(), Some(&mut normalize));
    let after = now();
    assert_eq!(output.len(), 3);
    let Message::ToolResult(real) = &output[2] else {
        panic!()
    };
    assert!(real.timestamp >= before && real.timestamp <= after);
    assert_eq!(real.timestamp.fract(), 0.0);
    assert_eq!(
        real,
        &ToolResultMessage {
            tool_call_id: "call_123_fc_123".into(),
            tool_name: "read".into(),
            content: vec![InputContent::Text(TextContent {
                text: "No result provided".into(),
                text_signature: None
            })],
            details: None,
            is_error: true,
            timestamp: real.timestamp
        }
    );
    let mut expected = serde_json::to_value(&messages).unwrap();
    expected[1]["content"][0]["id"] = json!("call_123_fc_123");
    assert_eq!(serde_json::to_value(&output[..2]).unwrap(), expected);
}

#[test]
fn copilot_repair_leaves_answered_trailing_call_alone() {
    let real: Message = serde_json::from_value(json!({"role":"toolResult","toolCallId":"call_1|fc_1","toolName":"read","content":[{"type":"text","text":"done"}],"isError":false,"timestamp":now()})).unwrap();
    let messages = vec![
        user("run commands"),
        Message::Assistant(assistant(json!([
            {"type":"toolCall","id":"call_1|fc_1","name":"read","arguments":{"path":"README.md"}},
            {"type":"toolCall","id":"call_2|fc_2","name":"bash","arguments":{"command":"pwd"}}
        ]))),
        real,
    ];
    let before = now();
    let output = transform_messages(&messages, &model(), Some(&mut normalize));
    let after = now();
    assert_eq!(
        output
            .iter()
            .filter(|m| matches!(m, Message::ToolResult(r) if r.is_error))
            .count(),
        1
    );
    let mut expected = serde_json::to_value(&messages).unwrap();
    expected[1]["content"][0]["id"] = json!("call_1_fc_1");
    expected[1]["content"][1]["id"] = json!("call_2_fc_2");
    expected[2]["toolCallId"] = json!("call_1_fc_1");
    assert_eq!(serde_json::to_value(&output[..3]).unwrap(), expected);
    let Message::ToolResult(r) = &output[3] else {
        panic!()
    };
    assert!(r.timestamp >= before && r.timestamp <= after);
    assert_eq!(
        r,
        &ToolResultMessage {
            tool_call_id: "call_2_fc_2".into(),
            tool_name: "bash".into(),
            content: vec![InputContent::Text(TextContent {
                text: "No result provided".into(),
                text_signature: None
            })],
            is_error: true,
            details: None,
            timestamp: r.timestamp
        }
    );
}

#[test]
fn normalizer_fixture_counts_utf16_units() {
    let source = assistant(json!([]));
    for (input, expected) in [("|é😀_-09Az", "_____-09Az"), ("", "")] {
        assert_eq!(normalize(input, &model(), &source), expected);
    }
    for count in [63, 64, 65] {
        assert_eq!(
            normalize(&"a".repeat(count), &model(), &source),
            "a".repeat(count.min(64))
        );
    }
    assert_eq!(
        normalize(&format!("{}😀", "a".repeat(63)), &model(), &source),
        format!("{}_", "a".repeat(63))
    );
    assert_eq!(
        transform_messages(&[], &model(), Some(&mut normalize)),
        vec![]
    );
}
