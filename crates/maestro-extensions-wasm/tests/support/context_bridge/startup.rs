//! Actions an authored factory calls before the host has bound them.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    ExtensionAPI, ExtensionResult, FlagOptions, FlagType, FlagValue, Model, ThinkingLevel,
    UserContent,
};
use serde::Serialize;
use serde_json::{Value, json};

/// Channel the factory reports each action outcome on.
const CHANNEL: &str = "startup-actions";

/// Calls every action while the factory still loads, reports each outcome, then returns.
pub(super) async fn observe(api: &ExtensionAPI) -> ExtensionResult<()> {
    api.register_flag(
        "startup-actions",
        FlagOptions {
            description: None,
            type_: FlagType::Boolean,
            default: None,
        },
    )?;
    if !matches!(
        api.get_flag("startup-actions")?,
        Some(FlagValue::Boolean(true))
    ) {
        return Ok(());
    }
    let message =
        serde_json::from_value(json!({"customType":"startup","content":"literal","display":false}))
            .map_err(|error| error.to_string())?;
    let outcomes = vec![
        outcome("sendMessage", api.send_message(message, None)),
        outcome(
            "sendUserMessage",
            api.send_user_message(UserContent::Text("literal".into()), None),
        ),
        outcome("appendEntry", api.append_entry("startup", None)),
        outcome("setSessionName", api.set_session_name("literal")),
        outcome("getSessionName", api.get_session_name()),
        outcome("setActiveTools", api.set_active_tools(&["literal".into()])),
        outcome("getThinkingLevel", api.get_thinking_level()),
        outcome(
            "setThinkingLevel",
            api.set_thinking_level(ThinkingLevel::High),
        ),
        outcome("setModel", api.set_model(model()?).await),
    ];
    api.events().emit(CHANNEL, Value::Array(outcomes)).await
}

/// Reports an action's value or the guidance that rejected it, unchanged.
fn outcome<T: Serialize>(name: &str, result: ExtensionResult<T>) -> Value {
    super::retention::observed(name, result.map(|value| json!(value)))
}

/// A complete model the factory offers to the rejected selection.
fn model() -> ExtensionResult<Model> {
    serde_json::from_value(json!({
        "id": "startup", "name": "startup", "api": "fixture", "provider": "fixture",
        "baseUrl": "https://example.invalid", "reasoning": false, "input": [],
        "cost": {"input":0,"output":0,"cacheRead":0,"cacheWrite":0},
        "contextWindow": 1000, "maxTokens": 10,
    }))
    .map_err(|error| error.to_string())
}
