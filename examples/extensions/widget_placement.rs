//! A startup extension that places text widgets above and below the editor.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::rc::Rc;

use maestro_extensions_wasm::{
    ExtensionAPI, ExtensionContext, ExtensionEvent, ExtensionEventResult, ExtensionFuture,
    ExtensionWidgetOptions, WidgetPlacement,
};

/// Requests both widgets for one session start, or neither when the host has no UI.
fn session_start(
    _event: &mut ExtensionEvent,
    context: ExtensionContext,
) -> ExtensionFuture<'_, Option<ExtensionEventResult>> {
    Box::pin(async move {
        if !context.has_ui()? {
            return Ok(None);
        }
        context.ui()?.set_widget(
            "widget-above",
            Some(&["Above editor widget".to_owned()]),
            None,
        )?;
        context.ui()?.set_widget(
            "widget-below",
            Some(&["Below editor widget".to_owned()]),
            Some(ExtensionWidgetOptions {
                placement: Some(WidgetPlacement::BelowEditor),
            }),
        )?;
        Ok(None)
    })
}

/// Registers the widget requests through the supplied extension host.
///
/// # Errors
/// The returned future returns the host's rejection when it refuses the `session_start`
/// handler registration.
#[must_use]
pub fn widget_placement_extension(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move { api.on("session_start", Rc::new(session_start)) })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(widget_placement_extension);
