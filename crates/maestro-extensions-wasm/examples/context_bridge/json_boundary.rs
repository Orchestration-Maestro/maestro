//! Semantic payload round-trips through the original native event registrations.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    ExtensionAPI, ExtensionEvent, ExtensionEventResult, ExtensionResult, SessionEvent,
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::rc::Rc;

/// Registers only payload families whose arbitrary fields cross the canonical boundary.
pub(super) fn register(api: &ExtensionAPI) -> ExtensionResult<()> {
    for name in [
        "before_provider_request",
        "before_agent_start",
        "tool_call",
        "tool_result",
        "tool_execution_start",
        "tool_execution_update",
        "tool_execution_end",
        "context",
        "message_end",
        "session_compact",
        "session_tree",
        "session_before_tree",
        "session_before_compact",
    ] {
        let events = api.events();
        api.on(
            name,
            Rc::new(move |event, _| {
                let events = events.clone();
                Box::pin(async move {
                    let (data, result) = observe(event)?;
                    events.emit("canonical-json", data).await?;
                    Ok(result)
                })
            }),
        )?;
    }
    Ok(())
}

/// Reads the concrete guest record and returns its declared result schema.
fn observe(event: &ExtensionEvent) -> ExtensionResult<(Value, Option<ExtensionEventResult>)> {
    let (data, result) = match event {
        ExtensionEvent::BeforeProviderRequest(event) => (
            encode(event)?,
            Some(ExtensionEventResult::BeforeProviderRequest(
                event.payload.clone(),
            )),
        ),
        ExtensionEvent::BeforeAgentStart(event) => (
            encode(event)?,
            Some(ExtensionEventResult::BeforeAgentStart(decode(
                json!({"message":{"customType":"boundary","content":"literal","display":false,"details":{"n":2.5}}}),
            )?)),
        ),
        ExtensionEvent::ToolCall(event) => (
            roundtrip::<maestro_extensions_wasm::CustomToolCallEvent>(encode(event)?)?,
            None,
        ),
        ExtensionEvent::ToolResult(event) => (
            tool_result(event)?,
            Some(ExtensionEventResult::ToolResult(decode(
                json!({"details":{"n":2.5}}),
            )?)),
        ),
        ExtensionEvent::ToolExecutionStart(event) => (encode(event)?, None),
        ExtensionEvent::ToolExecutionUpdate(event) => (encode(event)?, None),
        ExtensionEvent::ToolExecutionEnd(event) => (encode(event)?, None),
        ExtensionEvent::Context(event) => (
            encode(event)?,
            Some(ExtensionEventResult::Context(decode(
                json!({"messages":event.messages}),
            )?)),
        ),
        ExtensionEvent::MessageEnd(event) => (
            encode(event)?,
            Some(ExtensionEventResult::MessageEnd(decode(
                json!({"message":event.message}),
            )?)),
        ),
        ExtensionEvent::Session(event) => return session(event),
        _ => return Err("unexpected boundary event".into()),
    };
    Ok((data, result))
}

/// Checks both concrete entry aliases and resource-bearing summary results.
fn session(event: &SessionEvent) -> ExtensionResult<(Value, Option<ExtensionEventResult>)> {
    Ok(match event {
        SessionEvent::Compact(event) => (encode(event)?, None),
        SessionEvent::Tree(event) => (encode(event)?, None),
        SessionEvent::BeforeTree(event) => (
            json!({"preparation":event.preparation}),
            Some(ExtensionEventResult::SessionBeforeTree(decode(
                json!({"summary":{"summary":"literal","details":{"n":2.5}}}),
            )?)),
        ),
        SessionEvent::BeforeCompact(event) => (
            encode(&event.data)?,
            Some(ExtensionEventResult::SessionBeforeCompact(decode(
                json!({"compaction":{"summary":"literal","firstKeptEntryId":"kept","tokensBefore":2.5,"details":{"n":2.5}}}),
            )?)),
        ),
        _ => return Err("unexpected boundary session event".into()),
    })
}

/// Exercises each declared built-in input alias without replacing its typed details.
fn tool_result(event: &maestro_extensions_wasm::ToolResultEvent) -> ExtensionResult<Value> {
    let data = encode(event)?;
    match event.tool_name.as_str() {
        "bash" => roundtrip::<maestro_extensions_wasm::BashToolResultEvent>(data),
        "read" => roundtrip::<maestro_extensions_wasm::ReadToolResultEvent>(data),
        "edit" => roundtrip::<maestro_extensions_wasm::EditToolResultEvent>(data),
        "write" => roundtrip::<maestro_extensions_wasm::WriteToolResultEvent>(data),
        "grep" => roundtrip::<maestro_extensions_wasm::GrepToolResultEvent>(data),
        "find" => roundtrip::<maestro_extensions_wasm::FindToolResultEvent>(data),
        "ls" => roundtrip::<maestro_extensions_wasm::LsToolResultEvent>(data),
        _ => roundtrip::<maestro_extensions_wasm::CustomToolResultEvent>(data),
    }
}

/// Serializes the original received value, exposing semantic payloads to the native bus.
fn encode(value: &impl Serialize) -> ExtensionResult<Value> {
    serde_json::to_value(value).map_err(|error| error.to_string())
}

/// Constructs an authored result using its public record decoder.
fn decode<T: DeserializeOwned>(value: Value) -> ExtensionResult<T> {
    serde_json::from_value(value).map_err(|error| error.to_string())
}

/// Ensures specialized aliases retain the semantic payload received by the event union.
fn roundtrip<T: DeserializeOwned + Serialize>(value: Value) -> ExtensionResult<Value> {
    encode(&decode::<T>(value)?)
}
