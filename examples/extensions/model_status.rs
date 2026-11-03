//! A model-status extension that reports the model each selection chose.
// maestro-component: MAESTRO_MODEL_STATUS_COMPONENT
#![cfg(any(test, target_arch = "wasm32"))]
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::rc::Rc;

use maestro_extensions_wasm::{
    ExtensionAPI, ExtensionContext, ExtensionEvent, ExtensionEventResult, ExtensionFuture,
    ModelSelectSource, NotificationType,
};

/// Reports one model selection without changing the supplied event.
fn model_select(
    event: &mut ExtensionEvent,
    context: ExtensionContext,
) -> ExtensionFuture<'_, Option<ExtensionEventResult>> {
    Box::pin(async move {
        let ExtensionEvent::ModelSelect(event) = event else {
            return Ok(None);
        };
        let next = format!("{}/{}", event.model.provider, event.model.id);
        let prev = event.previous_model.present().map_or_else(
            || "none".to_owned(),
            |model| format!("{}/{}", model.provider, model.id),
        );
        let ui = context.ui()?;
        if event.source != ModelSelectSource::Restore {
            ui.notify(&format!("Model: {next}"), Some(NotificationType::Info))?;
        }
        ui.set_status("model", Some(&format!("🤖 {}", event.model.id)))?;
        let source = match event.source {
            ModelSelectSource::Set => "set",
            ModelSelectSource::Cycle => "cycle",
            ModelSelectSource::Restore => "restore",
        };
        println!("[model_select] {prev} → {next} ({source})");
        Ok(None)
    })
}

/// Registers the model-selection status handler through the supplied host.
///
/// # Errors
/// The returned future returns the host's rejection when it refuses the handler
/// registration.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move { api.on("model_select", Rc::new(model_select)) })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
