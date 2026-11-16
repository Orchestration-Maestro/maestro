//! Stream updates delivered through both author adapters.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::documents::{Answer, ask};
use super::scenario::Driver;
use serde::Serialize;
use serde_json::{Value, json};

/// A complete assistant record with intentionally unequal usage totals.
pub(super) fn assistant() -> Value {
    json!({"role":"assistant","content":[
        {"type":"thinking","thinking":"hidden","thinkingSignature":"sig Ω","redacted":true},
        {"type":"text","text":"answer","textSignature":"text sig"},
        {"type":"toolCall","id":"call","name":"tool","arguments":{"n":2},"thoughtSignature":"tool sig"}],
        "api":"openai-completions","provider":"custom","model":"m","responseModel":"actual","responseId":"response",
        "usage":{"input":1.0,"output":2.0,"cacheRead":3.0,"cacheWrite":4.0,"totalTokens":99.0,
        "cost":{"input":5.0,"output":6.0,"cacheRead":7.0,"cacheWrite":8.0,"total":101.0}},
        "diagnostics":[{"type":"custom","timestamp":9.0,"error":{"message":"error","name":"name","stack":"stack","code":"007"},"details":{"x":1}}],
        "stopReason":"length","timestamp":10.0})
}

/// Every stream shape retains its own payload and independent outer message.
async fn streams(driver: &mut impl Driver) -> Result<(), String> {
    for kind in [
        "start",
        "text_start",
        "text_delta",
        "text_end",
        "thinking_start",
        "thinking_delta",
        "thinking_end",
        "toolcall_start",
        "toolcall_delta",
        "toolcall_end",
        "done",
        "error",
    ] {
        let mut stream = json!({"type":kind});
        match kind {
            "done" => {
                stream["reason"] = json!("stop");
                stream["message"] = assistant();
            }
            "error" => {
                stream["reason"] = json!("aborted");
                stream["error"] = assistant();
            }
            _ => {
                stream["partial"] = assistant();
                if kind != "start" {
                    stream["contentIndex"] = json!(2);
                }
                if kind.ends_with("delta") {
                    stream["delta"] = json!(" delta Ω ");
                }
                if kind == "text_end" || kind == "thinking_end" {
                    stream["content"] = json!(" final ");
                }
                if kind == "toolcall_end" {
                    stream["toolCall"] = assistant()["content"][2].clone();
                }
            }
        }
        let event = json!({"type":"message_update","message":{"role":"user","content":"outer","timestamp":3.0},"assistantMessageEvent":stream});
        assert_eq!(
            ask(driver, &event, &json!({})).await?,
            Answer::returned(&event, None)
        );
    }
    Ok(())
}

/// Tool arguments and results are opaque text, not parsed JSON.
async fn tools(driver: &mut impl Driver) -> Result<(), String> {
    for text in [
        "null",
        "42",
        "\"text\"",
        " {\"x\":1,\"x\":2} ",
        "[]",
        "not JSON Ω",
    ] {
        for (kind, is_error) in [
            ("tool_execution_start", false),
            ("tool_execution_update", false),
            ("tool_execution_end", false),
            ("tool_execution_end", true),
        ] {
            let mut event = json!({"type":kind,"toolCallId":"id","toolName":"custom Ω"});
            if kind != "tool_execution_end" {
                event["args"] = json!(text);
            }
            if kind == "tool_execution_update" {
                event["partialResult"] = json!(text);
            }
            if kind == "tool_execution_end" {
                event["result"] = json!(text);
                event["isError"] = json!(is_error);
            }
            assert_eq!(
                ask(driver, &event, &json!({})).await?,
                Answer::returned(&event, None)
            );
        }
    }
    Ok(())
}

/// Mutates the retained original handle between validation and encoding.
struct Between {
    /// Detached output data.
    event: crate::types::EventData,
    /// Original handle retained by the callback.
    shared: crate::SharedAssistantMessage,
}
impl Serialize for Between {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let result = self.event.serialize(serializer);
        self.shared.write().unwrap().timestamp = f64::NAN;
        result
    }
}

/// Shared data is frozen before the finite and JSON serialization passes.
#[test]
fn maestro_stream_output_observes_one_post_callback_snapshot() -> Result<(), String> {
    use crate::bindings::exports::maestro::extension::guest::Decision;
    use crate::{AgentMessage, AssistantMessageEvent, ExtensionEvent, MessageUpdateEvent};
    let shared = std::sync::Arc::new(std::sync::RwLock::new(
        serde_json::from_value(assistant()).map_err(|e| e.to_string())?,
    ));
    let message: AgentMessage =
        serde_json::from_value(json!({"role":"user","content":"outer","timestamp":3.0}))
            .map_err(|e| e.to_string())?;
    let event = crate::types::EventData::from(ExtensionEvent::MessageUpdate(Box::new(
        MessageUpdateEvent {
            message,
            assistant_message_event: AssistantMessageEvent::Start {
                partial: shared.clone(),
            },
        },
    )));
    let result =
        crate::component_adapter::outcome(Some(Between { event, shared }), Ok(None::<Value>));
    let text = result.event?.ok_or("missing event")?;
    let value: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    assert_eq!(
        value["assistantMessageEvent"]["partial"]["timestamp"],
        json!(10.0)
    );
    assert!(matches!(result.decision, Decision::Returned(Ok(None))));
    Ok(())
}

/// Typed edits of nested stream numbers retain bits or fail before JSON output.
async fn numbers(driver: &mut impl Driver) -> Result<(), String> {
    let handler = driver.identity("event probe")?;
    for bits in [
        0,
        0x8000_0000_0000_0000,
        1,
        0x7fef_ffff_ffff_ffff,
        f64::INFINITY.to_bits(),
        f64::NEG_INFINITY.to_bits(),
        f64::NAN.to_bits(),
    ] {
        let event = super::documents::document("message_update");
        let directive =
            json!({"numberBits":format!("{bits:016x}"),"ending":{"fails":"authored failure"}});
        let delivery = driver
            .deliver(handler, &event.to_string(), &directive.to_string(), None)
            .await?;
        assert_eq!(
            delivery.decision,
            super::scenario::Decision::Failed("authored failure".into())
        );
        if f64::from_bits(bits).is_finite() {
            let returned: Value = serde_json::from_str(&delivery.event?.ok_or("missing event")?)
                .map_err(|e| e.to_string())?;
            assert_eq!(
                returned["assistantMessageEvent"]["partial"]["usage"]["totalTokens"]
                    .as_f64()
                    .unwrap()
                    .to_bits(),
                bits
            );
        } else {
            assert_eq!(
                delivery.event,
                Err("extension wrote a non-finite number (Infinity or NaN)".into())
            );
        }
    }
    Ok(())
}

/// Successful and failed stream terminal reasons have distinct admitted sets.
async fn terminal_reasons(driver: &mut impl Driver) -> Result<(), String> {
    let handler = driver.identity("event probe")?;
    for (kind, field, reasons, rejected) in [
        (
            "done",
            "message",
            &["stop", "length", "toolUse"][..],
            &["aborted", "error"][..],
        ),
        (
            "error",
            "error",
            &["aborted", "error"][..],
            &["stop", "length", "toolUse"][..],
        ),
    ] {
        for reason in reasons {
            let mut stream = json!({"type":kind,"reason":reason});
            stream[field] = assistant();
            let event = json!({"type":"message_update","message":{"role":"custom","customType":"outer","content":[],"display":true,"timestamp":2.0},"assistantMessageEvent":stream});
            assert_eq!(
                ask(driver, &event, &json!({})).await?,
                Answer::returned(&event, None)
            );
        }
        for reason in rejected {
            let mut stream = json!({"type":kind,"reason":reason});
            stream[field] = assistant();
            let event = json!({"type":"message_update","message":{"role":"user","content":"outer","timestamp":1.0},"assistantMessageEvent":stream});
            assert!(
                driver
                    .deliver(handler, &event.to_string(), "{}", None)
                    .await
                    .is_err()
            );
        }
    }
    Ok(())
}

#[test]
fn maestro_stream_terminal_reasons_keep_their_subsets() -> Result<(), String> {
    on_both_adapters!(terminal_reasons)
}

#[test]
fn maestro_nested_event_numbers_keep_bits() -> Result<(), String> {
    on_both_adapters!(numbers)
}

#[test]
fn maestro_tool_notifications_keep_opaque_values() -> Result<(), String> {
    on_both_adapters!(tools)
}

#[test]
fn maestro_stream_events_keep_all_variants_and_signatures() -> Result<(), String> {
    on_both_adapters!(streams)
}
