//! Callback-return ordering observed across independently loaded instances.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    CallbackEmitter, Component, EventBus, ExtensionAPI, ExtensionFuture, ExtensionResult,
    FlagOptions, FlagType, FlagValue,
};
use serde_json::{Value, json};
use std::rc::Rc;

/// Flag naming this instance in its trace entries; the native caller supplies it.
const LABEL: &str = "bridge-label";

/// Registers the ordering renderer and listeners that trace under the supplied label.
pub(crate) fn register(api: &ExtensionAPI) -> ExtensionResult<()> {
    api.register_flag(
        LABEL,
        FlagOptions {
            description: None,
            type_: FlagType::String,
            default: None,
        },
    )?;
    let renderer_api = api.clone();
    api.register_message_renderer(
        "order-view",
        Rc::new(move |message, _, _, emitter| {
            let label = label(&renderer_api)?;
            trace(&emitter, &label, "render:start")?;
            emitter.emit("order-inner", json!(label))?;
            emitter.emit("order-after", Value::Null)?;
            trace(&emitter, &label, "render:end")?;
            let name = message.custom_type;
            Ok(Some(super::components::with_release(
                super::ui::View {
                    api: renderer_api.clone(),
                    name: name.clone(),
                },
                &renderer_api,
                name,
            )))
        }),
    )?;
    probe(api)?;
    let inner_api = api.clone();
    api.events().on(
        "order-inner",
        Rc::new(move |sender, emitter| {
            let label = label(&inner_api)?;
            trace(&emitter, &label, "inner:start")?;
            if sender != json!(label) {
                emitter.emit("order-back", Value::Null)?;
            }
            trace(&emitter, &label, "inner:end")?;
            Ok(Some(tail(inner_api.events(), label, "inner:tail")))
        }),
    )?;
    let back_api = api.clone();
    api.events().on(
        "order-back",
        Rc::new(move |_, emitter| {
            let label = label(&back_api)?;
            trace(&emitter, &label, "back")?;
            Ok(Some(tail(back_api.events(), label, "back:tail")))
        }),
    )?;
    let after_api = api.clone();
    api.events().on(
        "order-after",
        Rc::new(move |_, emitter| {
            trace(&emitter, &label(&after_api)?, "after")?;
            Ok(None)
        }),
    )?;
    Ok(())
}

/// Registers a renderer whose views fail in the method its message type names.
fn probe(api: &ExtensionAPI) -> ExtensionResult<()> {
    api.register_message_renderer(
        "probe-view",
        Rc::new(|message, _, _, emitter| {
            let (category, error) = message
                .custom_type
                .split_once('|')
                .ok_or("missing failure category")?;
            emitter.emit("probe-foreign", json!("renderer"))?;
            if category == "renderer" {
                return Err(error.into());
            }
            Ok(Some(Component::new(Probe {
                category: category.into(),
                error: error.into(),
            })))
        }),
    )
}

/// Reads the label the native caller assigned to this instance.
pub(super) fn label(api: &ExtensionAPI) -> ExtensionResult<String> {
    match api.get_flag(LABEL)? {
        Some(FlagValue::String(label)) => Ok(label),
        _ => Err("missing instance label".into()),
    }
}

/// Emits one entry on this instance's own trace channel.
fn trace(emitter: &CallbackEmitter<'_>, label: &str, step: &str) -> ExtensionResult<()> {
    emitter.emit(&format!("trace:{label}"), json!(format!("{label}:{step}")))
}

/// Creates an owned continuation that traces after its prefix returned.
fn tail(events: EventBus, label: String, step: &'static str) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        events
            .emit(&format!("trace:{label}"), json!(format!("{label}:{step}")))
            .await
    })
}

/// A view whose selected method emits to other instances, then fails with the authored error.
struct Probe {
    /// Method that fails.
    category: String,
    /// Authored failure.
    error: String,
}

impl Probe {
    /// Emits for `method`, then fails when it is the selected one.
    fn answer(&self, method: &str, emitter: &CallbackEmitter<'_>) -> ExtensionResult<()> {
        emitter.emit("probe-foreign", json!(method))?;
        if self.category == method {
            return Err(self.error.clone());
        }
        Ok(())
    }
}

impl maestro_extensions_wasm::ComponentView for Probe {
    fn render(&self, width: u32, emitter: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        self.answer("render", &emitter)?;
        Ok(vec![format!("probe:{width}")])
    }
    fn handle_input(&self, _: &str, emitter: CallbackEmitter<'_>) -> ExtensionResult<()> {
        self.answer("input", &emitter)
    }
    fn wants_key_release(&self, emitter: CallbackEmitter<'_>) -> ExtensionResult<bool> {
        self.answer("key-release", &emitter)?;
        Ok(true)
    }
    fn invalidate(&self, emitter: CallbackEmitter<'_>) -> ExtensionResult<()> {
        self.answer("invalidate", &emitter)
    }
}
