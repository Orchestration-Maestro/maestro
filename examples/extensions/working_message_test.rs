//! A startup extension that requests a custom working text and indicator.
// maestro-component: MAESTRO_WORKING_MESSAGE_TEST_COMPONENT
#![cfg(any(test, target_arch = "wasm32"))]
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::rc::Rc;

use maestro_extensions_wasm::{
    ExtensionAPI, ExtensionContext, ExtensionEvent, ExtensionEventResult, ExtensionFuture,
    WorkingIndicatorOptions,
};

/// The working text this extension displays, with its colour and reset.
const CUSTOM_MESSAGE: &str = "\x1b[38;2;155;86;63mWorking... (custom)\x1b[39m";

/// The single indicator frame this extension displays, with its colour and reset.
const CUSTOM_INDICATOR: &str = "\x1b[38;2;155;86;63m●\x1b[39m";

/// Requests the custom working text and indicator whenever a session starts.
fn session_start(
    _event: &mut ExtensionEvent,
    context: ExtensionContext,
) -> ExtensionFuture<'_, Option<ExtensionEventResult>> {
    Box::pin(async move {
        let ui = context.ui()?;
        ui.set_working_message(Some(CUSTOM_MESSAGE))?;
        ui.set_working_indicator(Some(WorkingIndicatorOptions {
            frames: Some(vec![CUSTOM_INDICATOR.to_owned()]),
            interval_ms: None,
        }))?;
        Ok(None)
    })
}

/// Registers the startup handler through the supplied extension host.
///
/// # Errors
/// Returns the host rejection unchanged when the host refuses the registration.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move { api.on("session_start", Rc::new(session_start)) })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
