//! Agent notifications through both author adapters.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use super::documents::{Answer, ask};
use super::scenario::Driver;
use serde_json::json;

/// Delivers the agent lifecycle and assembled prompt records.
async fn agent_events(driver: &mut impl Driver) -> Result<(), String> {
    let user = json!({"role":"user","content":"original Ω", "timestamp":12.5});
    let tool = json!({"role":"toolResult","toolCallId":"id","toolName":"tool","content":[{"type":"image","data":"AA==","mimeType":"image/png"}],"isError":true,"timestamp":7.5});
    let assistant = json!({"role":"assistant","content":[{"type":"text","text":"answer","textSignature":"opaque"}],"api":"a","provider":"p","model":"m","usage":{"input":1.0,"output":2.0,"cacheRead":3.0,"cacheWrite":4.0,"totalTokens":99.0,"cost":{"input":1.0,"output":2.0,"cacheRead":3.0,"cacheWrite":4.0,"total":99.0}},"stopReason":"toolUse","timestamp":8.5});
    let other = json!({"role":"plugin","data":" {\"x\":1,\"x\":2} "});
    let options = json!({
        "cwd":" /../cwd ", "customPrompt":"custom", "selectedTools":["z","a","z"],
        "toolSnippets":{"z":"snippet","a":"second"}, "promptGuidelines":["b","a"],
        "appendSystemPrompt":"append", "contextFiles":[{"path":"../file","content":"body"}],
        "skills":[{"name":"skill", "description":"description", "filePath":"../skill",
            "baseDir":"..", "disableModelInvocation":false,
            "sourceInfo":{"path":"source", "source":"literal", "scope":"project",
                "origin":"top-level"}}]
    });
    let events = [
        json!({"type":"context", "messages":[tool.clone(), user.clone(), assistant.clone(), other, user.clone()]}),
        json!({"type":"before_agent_start", "prompt":"prompt", "images":[{"type":"image","data":"AA==", "mimeType":"image/png"}], "systemPrompt":"system", "systemPromptOptions":options}),
        json!({"type":"agent_start"}),
        json!({"type":"agent_end", "messages":[user.clone()]}),
        json!({"type":"turn_start", "turnIndex":-1.5, "timestamp":42.0}),
        json!({"type":"turn_end", "turnIndex":3.0, "message":user.clone(), "toolResults":[tool.clone(), tool]}),
        json!({"type":"message_start", "message":assistant}),
        json!({"type":"message_end", "message":user}),
    ];
    for event in events {
        assert_eq!(
            ask(driver, &event, &json!({})).await?,
            Answer::returned(&event, None)
        );
    }
    for kind in ["context", "agent_end"] {
        let event = json!({"type":kind,"messages":[]});
        assert_eq!(
            ask(driver, &event, &json!({})).await?,
            Answer::returned(&event, None)
        );
    }
    for optional in [json!({}), json!({"images":null}), json!({"images":[]})] {
        let mut event = json!({"type":"before_agent_start","prompt":"","systemPrompt":"","systemPromptOptions":{"cwd":""}});
        event
            .as_object_mut()
            .unwrap()
            .extend(optional.as_object().unwrap().clone());
        assert_eq!(
            ask(driver, &event, &json!({})).await?,
            Answer::returned(&event, None)
        );
    }
    Ok(())
}

#[test]
fn maestro_agent_events_keep_messages_and_system_prompt_options() -> Result<(), String> {
    on_both_adapters!(agent_events)
}
