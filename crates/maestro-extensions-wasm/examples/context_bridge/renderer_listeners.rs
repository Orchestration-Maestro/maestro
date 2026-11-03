//! Renderer-created listeners with independently retained asynchronous continuations.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use maestro_extensions_wasm::{ExtensionAPI, ExtensionFuture};
use std::rc::Rc;

/// Registers real same-instance listeners, including recursive synchronous emission.
#[cfg(target_arch = "wasm32")]
pub(super) fn register_listeners(api: &ExtensionAPI) -> Result<(), String> {
    for label in ["g1", "g2"] {
        let events = api.events();
        api.events().on(
            "renderer-created",
            Rc::new(move |data, emitter| {
                let name = data[0].as_str().ok_or("missing name")?;
                emitter.emit("trace", serde_json::json!(format!("{label}:{name}")))?;
                if label == "g1" && name == "listener-error" {
                    return Err("guest listener failure".into());
                }
                if name == "listener-tail" {
                    return Ok(Some(listener_tail(events.clone(), label)));
                }
                if label == "g1" && name == "reentry" {
                    emitter.emit("renderer-created", serde_json::json!(["inner", false]))?;
                }
                Ok(None)
            }),
        )?;
    }
    Ok(())
}

/// Creates a separately owned continuation for one listener prefix.
#[cfg(target_arch = "wasm32")]
fn listener_tail(
    events: maestro_extensions_wasm::EventBus,
    label: &'static str,
) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        events
            .emit("trace", serde_json::json!(format!("{label}:tail")))
            .await
    })
}
