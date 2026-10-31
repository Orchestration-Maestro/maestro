//! Owned standard records at the model boundary.
#![cfg(test)]
use maestro_agent::{AgentMessage, CustomAgentMessages};
use maestro_models::{Message, ToolResultMessage, UserContent, UserMessage};
use std::sync::{Arc, RwLock};

/// An unknown custom role.
struct Foreign;
impl CustomAgentMessages for Foreign {
    fn role(&self) -> &'static str {
        "foreign"
    }
}
#[test]
fn standard_message_projection_snapshots_known_variants() {
    let user = UserMessage {
        content: UserContent::Text("retained".into()),
        timestamp: 7.0,
    };
    let assistant = serde_json::from_value(serde_json::json!({"role":"assistant","content":[{"type":"text","text":"answer"}],"api":"openai-responses","provider":"custom","model":"fixture","usage":{"input":1,"output":2,"cacheRead":0,"cacheWrite":0,"totalTokens":3,"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}},"stopReason":"stop","timestamp":8.0})).unwrap();
    let tool = ToolResultMessage {
        tool_call_id: "call".into(),
        tool_name: "read".into(),
        content: vec![],
        details: None,
        is_error: true,
        timestamp: 9.0,
        usage: None,
    };
    let history: Vec<AgentMessage<Foreign>> = vec![
        AgentMessage::User(Arc::new(RwLock::new(user.clone()))),
        AgentMessage::Assistant(Arc::new(RwLock::new(assistant))),
        AgentMessage::ToolResult(Arc::new(RwLock::new(tool.clone()))),
        AgentMessage::Custom(Arc::new(RwLock::new(Foreign))),
    ];
    assert_eq!(
        serde_json::to_value(history[0].to_message().unwrap()).unwrap(),
        serde_json::to_value(Message::User(user)).unwrap()
    );
    let projected = history[1].to_message().unwrap();
    assert!(
        matches!(&projected,Message::Assistant(value) if value.model == "fixture" && value.timestamp.to_bits() == 8.0_f64.to_bits())
    );
    if let AgentMessage::Assistant(retained) = &history[1] {
        assert_eq!(
            serde_json::to_value(projected).unwrap(),
            serde_json::to_value(&*retained.read().unwrap()).unwrap()
        );
    }
    assert_eq!(
        serde_json::to_value(history[2].to_message().unwrap()).unwrap(),
        serde_json::to_value(Message::ToolResult(tool)).unwrap()
    );
    assert!(history[3].to_message().is_none());
}
