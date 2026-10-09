//! Agent notifications through both author adapters.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use super::documents::{Answer, ask};
use super::scenario::Driver;
use serde_json::{Value, json};

/// Ordered assembled resources supplied without discovery.
fn prompt_options() -> Value {
    json!({
        "cwd":" /../cwd ", "customPrompt":"custom", "selectedTools":["z","a","z"],
        "toolSnippets":{"z":"snippet","a":"second"}, "promptGuidelines":["b","a"],
        "appendSystemPrompt":"append", "contextFiles":[{"path":"../file","content":"body"}, {"path":"../a","content":"second"}],
        "skills":[{"name":"skill", "description":"description", "filePath":"../skill",
            "baseDir":"..", "disableModelInvocation":false,
            "sourceInfo":{"path":"source", "source":"literal", "scope":"project",
                "origin":"top-level"}},
            {"name":"another", "description":"second", "filePath":"../a",
                "baseDir":"../a", "disableModelInvocation":true,
                "sourceInfo":{"path":"another", "source":"second", "scope":"user",
                    "origin":"package"}}]
    })
}

/// Every accepted provenance scope and origin crosses both adapters.
async fn provenance(driver: &mut impl Driver) -> Result<(), String> {
    for scope in ["user", "project", "temporary"] {
        for origin in ["package", "top-level"] {
            let mut options = prompt_options();
            let source = &mut options["skills"][0]["sourceInfo"];
            source["scope"] = json!(scope);
            source["origin"] = json!(origin);
            let event = json!({"type":"before_agent_start", "prompt":"p", "systemPrompt":"s",
                "systemPromptOptions":options});
            assert_eq!(
                ask(driver, &event, &json!({})).await?,
                Answer::returned(&event, None)
            );
        }
    }
    Ok(())
}

/// Delivers the agent lifecycle and assembled prompt records.
async fn agent_events(driver: &mut impl Driver) -> Result<(), String> {
    let user = json!({"role":"user","content":"original Ω", "timestamp":12.5});
    let tool = json!({"role":"toolResult","toolCallId":"id","toolName":"tool","content":[{"type":"image","data":"AA==","mimeType":"image/png"}],"isError":true,"timestamp":7.5});
    let second_tool = json!({"role":"toolResult","toolCallId":"second","toolName":"another","content":[{"type":"text","text":"second result"}],"isError":false,"timestamp":9.0});
    let assistant = json!({"role":"assistant","content":[{"type":"text","text":"answer","textSignature":"opaque"}],"api":"a","provider":"p","model":"m","usage":{"input":1.0,"output":2.0,"cacheRead":3.0,"cacheWrite":4.0,"totalTokens":99.0,"cost":{"input":1.0,"output":2.0,"cacheRead":3.0,"cacheWrite":4.0,"total":99.0}},"stopReason":"toolUse","timestamp":8.5});
    let other = json!({"role":"plugin","data":" {\"x\":1,\"x\":2} "});
    let events = [
        json!({"type":"context", "messages":[tool.clone(), user.clone(), assistant.clone(), other, user.clone()]}),
        json!({"type":"before_agent_start", "prompt":"prompt", "images":[{"type":"image","data":"AA==", "mimeType":"image/png"}], "systemPrompt":"system", "systemPromptOptions":prompt_options()}),
        json!({"type":"agent_start"}),
        json!({"type":"agent_end", "messages":[user.clone()]}),
        json!({"type":"turn_start", "turnIndex":-1.5, "timestamp":42.0}),
        json!({"type":"turn_end", "turnIndex":3.0, "message":user.clone(), "toolResults":[tool.clone(), second_tool, tool]}),
        json!({"type":"message_start", "message":assistant}),
        json!({"type":"message_end", "message":user}),
    ];
    for event in events {
        let answer = ask(driver, &event, &json!({})).await?;
        if event["type"] == "before_agent_start" {
            let returned = answer.event.as_ref().ok_or("no event")?;
            let snippets = returned["systemPromptOptions"]["toolSnippets"]
                .as_object()
                .ok_or("no snippets")?;
            assert_eq!(
                snippets.keys().map(String::as_str).collect::<Vec<_>>(),
                ["z", "a"]
            );
        }
        assert_eq!(answer, Answer::returned(&event, None));
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
    provenance(driver).await
}

#[test]
fn maestro_agent_events_keep_messages_and_system_prompt_options() -> Result<(), String> {
    on_both_adapters!(agent_events)
}
