//! Canonical retained UI methods use the host's original capability.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    ExtensionContext, ExtensionResult, ExtensionWidgetOptions, NotificationType, WidgetPlacement,
    WorkingIndicatorOptions,
};
use serde_json::{Value, json};

/// Supplies distinguishable operands, then separately exercises host defaults and clearing.
pub(super) fn observe(context: &ExtensionContext, args: &str) -> ExtensionResult<String> {
    if let Some(operation) = args.strip_prefix("editor ") {
        return editor(context, operation);
    }
    let clear = args == "ui clear";
    let ui = context.ui()?;
    ui.notify(
        " literal notification ",
        if clear {
            None
        } else {
            Some(NotificationType::Warning)
        },
    )?;
    ui.set_status(" literal key ", (!clear).then_some(""))?;
    ui.set_working_message((!clear).then_some(" literal working "))?;
    ui.set_working_visible(!clear)?;
    ui.set_working_indicator((!clear).then(|| WorkingIndicatorOptions {
        frames: Some(vec![]),
        interval_ms: Some(2.5),
    }))?;
    ui.set_hidden_thinking_label((!clear).then_some(" literal hidden "))?;
    let lines = vec![" literal line ".into(), String::new()];
    ui.set_widget(
        " literal widget ",
        (!clear).then_some(lines.as_slice()),
        (!clear).then_some(ExtensionWidgetOptions {
            placement: Some(WidgetPlacement::BelowEditor),
        }),
    )?;
    ui.set_title(if clear { "" } else { " literal title " })?;
    Ok("UI forwarded".into())
}

/// Shows the dialog `request` names on the originally acquired UI and returns its answer.
pub(super) async fn dialog(context: &ExtensionContext, request: &str) -> ExtensionResult<String> {
    let request: Value = serde_json::from_str(request).map_err(|error| error.to_string())?;
    let text = |key: &str| request[key].as_str().map(str::to_owned);
    let title = text("title").unwrap_or_default();
    let ui = context.ui()?;
    let answer = match request["method"].as_str() {
        Some("select") => {
            let options = serde_json::from_value(request["options"].clone());
            let options = options.map_err(|error| error.to_string())?;
            json!(ui.select(title, options, None).await?)
        }
        Some("confirm") => {
            let message = text("message").unwrap_or_default();
            json!(ui.confirm(title, message, None).await?)
        }
        Some("input") => json!(ui.input(title, text("placeholder"), None).await?),
        _ => json!(ui.editor(title, text("prefill")).await?),
    };
    Ok(answer.to_string())
}

/// Reads and edits the originally acquired UI without interpreting the supplied text.
fn editor(context: &ExtensionContext, operation: &str) -> ExtensionResult<String> {
    let ui = context.ui()?;
    if let Some(text) = operation.strip_prefix("paste:") {
        ui.paste_to_editor(text)?;
    } else if let Some(text) = operation.strip_prefix("set:") {
        ui.set_editor_text(text)?;
    } else {
        return ui.get_editor_text();
    }
    Ok("editor forwarded".into())
}
