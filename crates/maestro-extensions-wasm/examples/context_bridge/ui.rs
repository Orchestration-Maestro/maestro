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
