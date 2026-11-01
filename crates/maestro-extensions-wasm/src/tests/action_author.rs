//! Ordinary actions exercised through the authored facade.
use maestro_extensions_wasm::{
    ExtensionAPI, ExtensionResult, MessageDelivery, SendMessageOptions, SendUserMessageOptions,
    UserMessageDelivery,
};
use serde_json::{Value, json};
/// Optional authored custom-message delivery choices.
fn custom_options(value: &Value) -> ExtensionResult<Option<SendMessageOptions>> {
    if !value["present"].as_bool().unwrap_or(false) {
        return Ok(None);
    }
    let value = &value["value"];
    let deliver_as = match value["deliverAs"].as_str() {
        None => None,
        Some("steer") => Some(MessageDelivery::Steer),
        Some("followUp") => Some(MessageDelivery::FollowUp),
        Some("nextTurn") => Some(MessageDelivery::NextTurn),
        _ => return Err("invalid authored delivery".into()),
    };
    Ok(Some(SendMessageOptions {
        trigger_turn: value["triggerTurn"].as_bool(),
        deliver_as,
    }))
}
/// Dispatch one ordinary action; the host supplies decisions, not the author.
pub async fn run(api: &ExtensionAPI, method: &str, input: &Value) -> ExtensionResult<Value> {
    match method {
        "sendMessage" => send_custom(api, input)?,
        "sendUserMessage" => send_user(api, input)?,
        "appendEntry" => api.append_entry(
            input["customType"].as_str().unwrap_or("state Ω"),
            input["data"]["present"]
                .as_bool()
                .unwrap_or(false)
                .then(|| input["data"]["value"].clone()),
        )?,
        "setSessionName" => api.set_session_name(input["name"].as_str().unwrap_or(""))?,
        "getSessionName" => return session_name(api),
        "setLabel" => api.set_label(
            input["entryId"].as_str().unwrap_or(""),
            input["label"]["present"]
                .as_bool()
                .unwrap_or(false)
                .then(|| input["label"]["value"].as_str().unwrap()),
        )?,
        "getActiveTools" => return Ok(json!(api.get_active_tools()?)),
        "setActiveTools" => api.set_active_tools(
            &serde_json::from_value::<Vec<String>>(input["names"].clone())
                .map_err(|e| e.to_string())?,
        )?,
        "getAllTools" => return tools(api, input),
        "getCommands" => return commands(api, input),
        "getThinkingLevel" => {
            return serde_json::to_value(api.get_thinking_level()?).map_err(|e| e.to_string());
        }
        "setThinkingLevel" => api.set_thinking_level(
            serde_json::from_value(input["level"].clone()).map_err(|e| e.to_string())?,
        )?,
        "setModel" => return select_model(api, input).await,
        _ => return Err("unknown ordinary action".into()),
    }
    Ok(Value::Null)
}
/// Run one request and record its catchable outcome.
pub async fn dispatch(api: &ExtensionAPI, request: &str) -> ExtensionResult<()> {
    let value: Value = serde_json::from_str(request).map_err(|e| e.to_string())?;
    if value["method"] == "control" {
        return api.append_entry("action-control", Some(value["input"].clone()));
    }
    let result = run(
        api,
        value["method"].as_str().ok_or("missing method")?,
        &value["input"],
    )
    .await;
    api.append_entry(
        "action-answer",
        Some(match result {
            Ok(value) => json!({"ok":value}),
            Err(error) => json!({"error":error}),
        }),
    )
}

/// Execute the sendMessage authored branch.
fn send_custom(api: &ExtensionAPI, input: &Value) -> ExtensionResult<()> {
    let message = input["message"].clone();
    api.send_message(
        serde_json::from_value(message).map_err(|e| e.to_string())?,
        custom_options(&input["options"])?,
    )?;

    Ok(())
}

/// Execute the sendUserMessage authored branch.
fn send_user(api: &ExtensionAPI, input: &Value) -> ExtensionResult<()> {
    let options = input["options"]["present"]
        .as_bool()
        .unwrap_or(false)
        .then(|| SendUserMessageOptions {
            deliver_as: match input["options"]["value"]["deliverAs"].as_str() {
                Some("steer") => Some(UserMessageDelivery::Steer),
                Some("followUp") => Some(UserMessageDelivery::FollowUp),
                _ => None,
            },
        });
    api.send_user_message(
        serde_json::from_value(input["content"].clone()).map_err(|e| e.to_string())?,
        options,
    )?;

    Ok(())
}

/// Execute the getAllTools authored branch.
fn tools(api: &ExtensionAPI, input: &Value) -> ExtensionResult<Value> {
    let tools = api.get_all_tools()?;
    if input["code"] == true {
        return Ok(json!(
            tools
                .iter()
                .map(|tool| json!({
                    "name":tool.name, "parameters":tool.parameters.as_json(),
                    "codeNodes":tool.parameters.code().len(),
                }))
                .collect::<Vec<_>>()
        ));
    }
    if input["kinds"] == true {
        return Ok(json!(
            tools
                .iter()
                .map(|tool| json!({
                    "parameters": tool.parameters.as_json(),
                    "kinds": tool.parameters.conversions(),
                }))
                .collect::<Vec<_>>()
        ));
    }
    serde_json::to_value(tools).map_err(|e| e.to_string())
}

/// Execute the getCommands authored branch.
fn commands(api: &ExtensionAPI, input: &Value) -> ExtensionResult<Value> {
    if input["raw"] == true {
        return serde_json::to_value(api.get_commands()?).map_err(|e| e.to_string());
    }
    let mut value = serde_json::to_value(api.get_commands()?).map_err(|e| e.to_string())?;
    for command in value.as_array_mut().unwrap() {
        if command["description"].is_null() {
            command.as_object_mut().unwrap().shift_remove("description");
        }
    }
    Ok(value)
}

/// Run the getSessionName authored branch.
fn session_name(api: &ExtensionAPI) -> ExtensionResult<Value> {
    Ok(match api.get_session_name()? {
        None => json!({"present":false}),
        Some(v) => json!({"present":true,"value":v}),
    })
}

/// Run the setModel authored branch.
async fn select_model(api: &ExtensionAPI, input: &Value) -> ExtensionResult<Value> {
    let mut model: maestro_extensions_wasm::Model =
        serde_json::from_value(input["model"].clone()).map_err(|e| e.to_string())?;
    if input["nonfinite"] == true {
        model.cost.input = f64::INFINITY;
    }
    Ok(json!(api.set_model(model).await?))
}
