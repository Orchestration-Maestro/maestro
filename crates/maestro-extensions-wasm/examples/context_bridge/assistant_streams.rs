//! Assistant events retain authored shared messages through the native generic buffer.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    AssistantMessageEvent as Event, AssistantMessageEventStream, DoneReason, ErrorReason,
    ExtensionResult, SharedAssistantMessage, create_assistant_message_event_stream,
};
use serde_json::{Value, json};
use std::sync::{Arc, RwLock};

/// Retain all message fields, including open diagnostics and tool-call arguments.
fn message(reason: &str) -> ExtensionResult<SharedAssistantMessage> {
    let value = json!({
        "role":"assistant", "api":"authored-api", "provider":"authored-provider", "model":"雪-model",
        "responseModel":"response-model", "responseId":"response-id",
        "content":[
            {"type":"text","text":"雪\n\u{0000}\u{001b}","textSignature":"text-signature"},
            {"type":"thinking","thinking":"考え","thinkingSignature":"thinking-signature","redacted":false},
            {"type":"toolCall","id":"call-id","name":"tool","arguments":{"n":-0.0,"roots":[null,false,0,"",{},[]]},"thoughtSignature":"tool-signature"}
        ],
        "usage":{"input":1.5,"output":2.5,"cacheRead":3.5,"cacheWrite":4.5,"totalTokens":null,
            "cost":{"input":0.125,"output":0.25,"cacheRead":0.5,"cacheWrite":1.0,"total":1.875}},
        "diagnostics":[{"type":"controlled","timestamp":6.25,
            "error":{"name":"ControlledError","message":"literal failure 雪","stack":"line\ntrace","code":17.5},
            "details":{"roots":[null,false,0,"",{},[]],"negativeZero":-0.0}}],
        "stopReason":reason,"errorMessage":"literal terminal detail 雪\n", "timestamp":-0.0,
    });
    Ok(Arc::new(RwLock::new(
        serde_json::from_value(value).map_err(|error| error.to_string())?,
    )))
}

/// Build every nonterminal variant without reconstructing content from deltas.
fn updates(partial: &SharedAssistantMessage) -> ExtensionResult<Vec<Event>> {
    let mut events = Vec::new();
    for value in [
        json!({"type":"start"}),
        json!({"type":"text_start","contentIndex":2}),
        json!({"type":"text_delta","contentIndex":2,"delta":"雪\n"}),
        json!({"type":"text_end","contentIndex":2,"content":"finished text"}),
        json!({"type":"thinking_start","contentIndex":3}),
        json!({"type":"thinking_delta","contentIndex":3,"delta":"考え"}),
        json!({"type":"thinking_end","contentIndex":3,"content":"finished thinking"}),
        json!({"type":"toolcall_start","contentIndex":4}),
        json!({"type":"toolcall_delta","contentIndex":4,"delta":"{\"n\":"}),
        json!({"type":"toolcall_end","contentIndex":4,"toolCall":{"type":"toolCall","id":"delta-id","name":"delta-tool","arguments":[false,null,0],"thoughtSignature":"delta-signature"}}),
    ] {
        let mut value = value;
        value["partial"] = serde_json::to_value(partial).map_err(|error| error.to_string())?;
        let mut event: Event = serde_json::from_value(value).map_err(|error| error.to_string())?;
        match &mut event {
            Event::Start { partial: handle }
            | Event::TextStart {
                partial: handle, ..
            }
            | Event::TextDelta {
                partial: handle, ..
            }
            | Event::TextEnd {
                partial: handle, ..
            }
            | Event::ThinkingStart {
                partial: handle, ..
            }
            | Event::ThinkingDelta {
                partial: handle, ..
            }
            | Event::ThinkingEnd {
                partial: handle, ..
            }
            | Event::ToolcallStart {
                partial: handle, ..
            }
            | Event::ToolcallDelta {
                partial: handle, ..
            }
            | Event::ToolcallEnd {
                partial: handle, ..
            } => *handle = partial.clone(),
            Event::Done { .. } | Event::Error { .. } => {
                return Err("unexpected terminal update".into());
            }
        }
        events.push(event);
    }
    Ok(events)
}

/// Select each distinct terminal reason with the supplied original message owner.
fn terminal(reason: &str, message: SharedAssistantMessage) -> ExtensionResult<Event> {
    Ok(match reason {
        "stop" => Event::Done {
            reason: DoneReason::Stop,
            message,
        },
        "length" => Event::Done {
            reason: DoneReason::Length,
            message,
        },
        "toolUse" => Event::Done {
            reason: DoneReason::ToolUse,
            message,
        },
        "error" => Event::Error {
            reason: ErrorReason::Error,
            error: message,
        },
        "aborted" => Event::Error {
            reason: ErrorReason::Aborted,
            error: message,
        },
        _ => return Err("unknown terminal reason".into()),
    })
}

/// Observe every update after mutating its still-shared partial message.
async fn all_events() -> ExtensionResult<Value> {
    let stream = AssistantMessageEventStream::new()?;
    let partial = message("stop")?;
    let source = stream.clone();
    for event in updates(&partial)? {
        source.push(event)?;
    }
    drop(source);
    partial.write().map_err(|error| error.to_string())?.model = "live-partial".into();
    let final_message = message("stop")?;
    stream.push(terminal("stop", final_message.clone())?)?;
    let result = stream.result().await?;
    let mut observed = Vec::new();
    let mut live_owner = true;
    while let Some(event) = stream.next().await? {
        if !event.is_terminal() {
            live_owner &= Arc::ptr_eq(event.message(), &partial)
                && event
                    .message()
                    .read()
                    .map_err(|error| error.to_string())?
                    .model
                    == "live-partial";
        }
        let mut value = serde_json::to_value(event).map_err(|error| error.to_string())?;
        let object = value.as_object_mut().ok_or("event was not an object")?;
        object.shift_remove("partial");
        object.shift_remove("message");
        observed.push(value);
    }
    Ok(
        json!({"updates":observed,"liveOwner":live_owner,"resultOwner":Arc::ptr_eq(&result,&final_message),"eof":stream.next().await?.is_none()}),
    )
}

/// Exercise successful and failed terminal reasons independently of event draining.
async fn reasons() -> ExtensionResult<Value> {
    let mut observed = Vec::new();
    for reason in ["stop", "length", "toolUse", "error", "aborted"] {
        let stream = create_assistant_message_event_stream()?;
        let selected = message(reason)?;
        stream.push(terminal(reason, selected.clone())?)?;
        let first = stream.result().await?;
        stream.end(Some(message("stop")?))?;
        let repeated = stream.result().await?;
        let event = stream.next().await?.ok_or("missing terminal event")?;
        let encoded = serde_json::to_value(event.as_ref()).map_err(|error| error.to_string())?;
        observed.push(json!({
            "kind":encoded["type"],"reason":encoded["reason"],"resultOwner":Arc::ptr_eq(&first,&selected),
            "sameOwner":Arc::ptr_eq(&first,&repeated),"eventOwner":Arc::ptr_eq(event.message(),&selected),
            "eof":stream.next().await?.is_none(),
        }));
    }
    Ok(json!(observed))
}

/// Combine complete event-variant and terminal-reason observations for the native consumer.
pub async fn events() -> ExtensionResult<Value> {
    Ok(json!({"events":all_events().await?,"reasons":reasons().await?}))
}

/// Read complete selected messages at the result consumer, not only from nearby events.
pub async fn results() -> ExtensionResult<Value> {
    let mut observed = Vec::new();
    for diagnostics in [true, false] {
        for reason in ["stop", "length", "toolUse", "error", "aborted"] {
            let selected = message(reason)?;
            if !diagnostics {
                let mut message = selected.write().map_err(|error| error.to_string())?;
                message.diagnostics = None;
                message.error_message = None;
                message.response_model = None;
                message.response_id = None;
            }
            let stream = create_assistant_message_event_stream()?;
            stream.push(terminal(reason, selected.clone())?)?;
            let result = stream.result().await?;
            let repeated = stream.result().await?;
            let queued = stream
                .next()
                .await?
                .ok_or("result observation drained its event")?;
            if !Arc::ptr_eq(queued.message(), &result) {
                return Err("terminal event and result lost their shared message".into());
            }
            drop(stream);
            observed.push(json!({"diagnostics":diagnostics,"message":result,
                "sameOwner":Arc::ptr_eq(&selected,&repeated)}));
        }
    }
    Ok(json!(observed))
}
