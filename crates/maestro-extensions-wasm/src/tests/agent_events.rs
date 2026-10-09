//! Agent notifications through both author adapters.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use super::documents::{Answer, ask, product, returns, strings};
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

/// Result families retain independent missing, null and replacement fields.
async fn results(driver: &mut impl Driver) -> Result<(), String> {
    let user = json!({"role":"user","content":"replacement","timestamp":99.0});
    for (kind, field, populated) in [
        ("context", "messages", json!([user.clone(), user.clone()])),
        (
            "message_end",
            "message",
            json!({"role":"branchSummary","summary":"replacement","fromId":"ancestor","timestamp":99.0}),
        ),
    ] {
        let event = super::documents::document(kind);
        assert_eq!(
            ask(driver, &event, &json!({})).await?,
            Answer::returned(&event, None)
        );
        let mut values = vec![json!({}), json!({field:null}), json!({field:populated})];
        if kind == "context" {
            values.push(json!({"messages":[]}));
        }
        for value in values {
            assert_eq!(
                ask(driver, &event, &returns(kind, &value)).await?,
                Answer::returned(&event, Some(value))
            );
        }
    }
    before_agent_results(driver).await
}

/// Before-agent custom content omits role and timestamp.
async fn before_agent_results(driver: &mut impl Driver) -> Result<(), String> {
    let event = super::documents::document("before_agent_start");
    let blocks = json!({"message":{"customType":"blocks","content":[],"display":false}});
    assert_eq!(
        ask(driver, &event, &returns("before_agent_start", &blocks)).await?,
        Answer::returned(&event, Some(blocks))
    );
    for [message, prompt] in product::<2>(&strings()) {
        let mut value = json!({});
        if let Some(message) = message {
            value["message"] = if message.is_null() {
                message
            } else {
                json!({"customType":"notice","content":message,"display":false,"details":" opaque "})
            };
        }
        if let Some(prompt) = prompt {
            value["systemPrompt"] = prompt;
        }
        assert_eq!(
            ask(driver, &event, &returns("before_agent_start", &value)).await?,
            Answer::returned(&event, Some(value))
        );
    }
    Ok(())
}

/// Checks the numeric result independently of event serialization.
fn assert_result_number(
    family: &str,
    bits: u64,
    decision: super::scenario::Decision,
) -> Result<(), String> {
    use super::scenario::Decision;
    if f64::from_bits(bits).is_finite() {
        let Decision::Returned(Ok(Some(text))) = decision else {
            return Err("missing result".into());
        };
        let value: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
        let timestamp = if family == "context" {
            &value["messages"][0]["timestamp"]
        } else {
            &value["message"]["timestamp"]
        };
        assert_eq!(timestamp.as_f64().unwrap().to_bits(), bits);
    } else {
        assert_eq!(
            decision,
            Decision::Returned(Err(
                "extension wrote a non-finite number (Infinity or NaN)".into()
            ))
        );
    }
    Ok(())
}

/// Nonfinite values assigned in new results fail independently of event output.
async fn result_numbers(driver: &mut impl Driver) -> Result<(), String> {
    use super::scenario::Decision;
    let handler = driver.identity("event probe")?;
    for family in ["context", "message_end"] {
        let message = json!({"role":"user","content":"assigned","timestamp":0.0});
        let result = if family == "context" {
            json!({"messages":[message]})
        } else {
            json!({"message":message})
        };
        for bits in [
            0x8000_0000_0000_0000,
            1,
            f64::INFINITY.to_bits(),
            f64::NEG_INFINITY.to_bits(),
            f64::NAN.to_bits(),
        ] {
            let event = super::documents::document(family);
            let mut directive = returns(family, &result);
            directive["resultNumberBits"] = json!(format!("{bits:016x}"));
            let delivery = driver
                .deliver(handler, &event.to_string(), &directive.to_string(), None)
                .await?;
            assert_eq!(
                Answer::of(super::scenario::Delivery {
                    event: delivery.event,
                    decision: Decision::Returned(Ok(None))
                })?
                .event,
                Some(event)
            );
            assert_result_number(family, bits, delivery.decision)?;
        }
    }
    Ok(())
}

#[test]
fn maestro_agent_result_numbers_validate_guest_assignments() -> Result<(), String> {
    on_both_adapters!(result_numbers)
}

#[test]
fn maestro_context_and_message_results_keep_presence() -> Result<(), String> {
    on_both_adapters!(results)
}

#[test]
fn maestro_agent_events_keep_messages_and_system_prompt_options() -> Result<(), String> {
    on_both_adapters!(agent_events)
}
