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
pub(super) fn observe(context: &ExtensionContext, clear: bool) -> ExtensionResult<()> {
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
    ui.set_title(if clear { "" } else { " literal title " })
}
