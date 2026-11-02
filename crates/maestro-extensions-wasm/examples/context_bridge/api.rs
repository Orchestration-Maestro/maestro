//! Canonical API operands sent through the original registration facade.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{ExtensionAPI, ExtensionCommandContext, ExtensionResult};
use serde_json::{Value, json};

/// Writes real session data and reads independently supplied application state.
pub(super) async fn observe(
    api: &ExtensionAPI,
    context: &ExtensionCommandContext,
    root: &str,
) -> ExtensionResult<Value> {
    api.register_flag(
        "canonical-api",
        maestro_extensions_wasm::FlagOptions {
            description: Some(" literal flag ".into()),
            type_: maestro_extensions_wasm::FlagType::Boolean,
            default: Some(maestro_extensions_wasm::FlagValue::Boolean(false)),
        },
    )?;
    api.append_entry("canonical-api", Some(json!({"nested": [2.5, null]})))?;
    api.set_session_name(" literal API name ")?;
    api.set_label(root, Some(" literal API label "))?;
    api.set_active_tools(&[" z ".into(), "a".into(), "a".into()])?;
    api.set_thinking_level(maestro_extensions_wasm::ThinkingLevel::High)?;
    let model = context.model()?.ok_or("missing API model")?;
    let auth = matches!(context.model_registry()?.get_api_key_and_headers(model.clone()).await?,
        maestro_extensions_wasm::ResolvedRequestAuth::Ok { api_key: Some(key), headers: None, ..} if key == "literal controlled key");
    let selected = api.set_model(model).await?;
    messages(api)?;
    super::registrations::register(api)?;
    super::json_boundary::register(api)?;
    unsubscribe(api).await?;
    Ok(json!({
        "name": api.get_session_name()?, "tools": api.get_active_tools()?,
        "all": api.get_all_tools()?, "commands": api.get_commands()?,
        "thinking": api.get_thinking_level()?, "selected": selected, "auth":auth,
        "flag": matches!(api.get_flag("canonical-api")?, Some(maestro_extensions_wasm::FlagValue::Boolean(false))),
    }))
}

/// Proves removal of a guest listener affects the next emission on the original bus.
async fn unsubscribe(api: &ExtensionAPI) -> ExtensionResult<()> {
    let events = api.events();
    let listener = events.on(
        "canonical-subscription",
        std::rc::Rc::new(|value, emitter| {
            emitter.emit("canonical-observer", value)?;
            Ok(None)
        }),
    )?;
    events
        .emit("canonical-subscription", json!({"present":true}))
        .await?;
    listener.unsubscribe()?;
    events
        .emit("canonical-subscription", json!({"present":false}))
        .await
}

/// Preserves semantic message payloads and independently optional delivery fields.
fn messages(api: &ExtensionAPI) -> ExtensionResult<()> {
    api.send_message(
        serde_json::from_value(json!({"customType":"canonical-api", "content":" literal custom ", "display":false,"details":{"n":2.5}}))
            .map_err(|error| error.to_string())?,
        Some(maestro_extensions_wasm::SendMessageOptions {
            trigger_turn: Some(false),
            deliver_as: Some(maestro_extensions_wasm::MessageDelivery::NextTurn),
        }),
    )?;
    api.send_user_message(
        serde_json::from_value(json!([{"type":"text", "text":" literal user "}]))
            .map_err(|error| error.to_string())?,
        Some(maestro_extensions_wasm::SendUserMessageOptions {
            deliver_as: Some(maestro_extensions_wasm::UserMessageDelivery::FollowUp),
        }),
    )
}
