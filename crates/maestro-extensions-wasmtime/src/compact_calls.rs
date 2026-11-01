//! Compaction callbacks retain their original guest identity and native event bus.
use crate::{
    api_reads::Endpoint,
    guest_calls::{Handle, SynchronousKind},
    local_resources::Registry,
};
use maestro_extensions::{CompactOptions, EventBus, Local};
use std::rc::Rc;
use wasmtime::component::{ComponentType, Lift};

/// Canonical options retain outer absence independently of absent instructions.
#[derive(ComponentType, Lift)]
#[component(record)]
pub(crate) struct Data {
    /// Literal authored instructions, without whitespace normalization.
    #[component(name = "custom-instructions")]
    pub(crate) custom_instructions: Option<String>,
}

/// Owned import operands before selecting the native compaction action.
pub(crate) struct Compact {
    /// Optional plain options, never replaced with a default record.
    pub(crate) data: Option<Data>,
    /// Original authored completion callback.
    pub(crate) on_complete: Option<Handle>,
    /// Original authored error callback.
    pub(crate) on_error: Option<Handle>,
    /// Supplies the original extension identity for native callback emission.
    pub(crate) api: Endpoint,
}

impl Compact {
    /// Constructs native notifications without changing when the native owner calls them.
    pub(crate) fn native(self, registry: &Registry) -> Result<Option<CompactOptions>, String> {
        let Some(data) = self.data else {
            return Ok(None);
        };
        let events = registry.events(&self.api)?;
        let complete_events = events.clone();
        let complete_registry = registry.clone();
        let on_complete = self.on_complete.map(|handler| {
            Rc::new(move |result: maestro_extensions::CompactionResult| {
                let data = serde_json::to_string(&result)?;
                notify(
                    &handler,
                    &complete_registry,
                    &complete_events,
                    SynchronousKind::CompactionComplete,
                    data,
                )
                .map_err(Into::into)
            }) as _
        });
        let error_registry = registry.clone();
        let on_error = self.on_error.map(|handler| {
            Rc::new(move |error: maestro_extensions::EventError| {
                notify(
                    &handler,
                    &error_registry,
                    &events,
                    SynchronousKind::CompactionError,
                    error.to_string(),
                )
                .map_err(Into::into)
            }) as _
        });
        Ok(Some(CompactOptions {
            custom_instructions: data.custom_instructions,
            on_complete,
            on_error,
        }))
    }
}

/// Uses the original native invocation frame for every synchronous notification.
pub(crate) fn notify(
    handler: &Handle,
    registry: &Registry,
    events: &EventBus<Local>,
    kind: SynchronousKind,
    data: String,
) -> Result<(), String> {
    events.invoke_callback(|emitter| {
        let emitter = registry.emitter(emitter.frame())?;
        let result = handler.synchronous(registry, kind, data, emitter.clone());
        drop(registry.take_tail(&emitter)?);
        result
    })
}
