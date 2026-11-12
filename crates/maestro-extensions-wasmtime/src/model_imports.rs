//! Typed model invocation resources over the existing JSON data transport.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{
    bindings::maestro::extension::host::HostAssistantStream,
    callback_imports::{CallbackView, retain},
    context_imports::add_resource,
    guest_calls::Callback,
    lifecycle::State,
    local_resources::{SignalEndpoint, model_streams::Endpoint},
    model_calls::Controls,
    read_imports::{endpoint, forwards, release},
};
use wasmtime::{
    StoreContextMut,
    component::{ComponentType, Lift, LinkerInstance, Resource},
};

/// Borrowed non-data invocation operands; every selected owner is retained before returning.
#[derive(ComponentType, Lift)]
#[component(record)]
struct WireControls {
    /// Original arbitrary caller cancellation.
    signal: Option<Resource<SignalEndpoint>>,
    /// Original payload selector.
    payload: Option<Resource<Callback>>,
    /// Original response observer.
    response: Option<Resource<Callback>>,
    /// Opaque controls and caller bindings are values, released only at Store quiescence.
    objects: Option<Resource<Callback>>,
}
impl WireControls {
    /// Retain capabilities before invoking their owner, without hiding them in JSON.
    fn retained<T: CallbackView>(self, state: &mut T) -> wasmtime::Result<Controls> {
        Ok(Controls {
            signal: self
                .signal
                .map(|signal| state.ctx().table.get(&signal).cloned())
                .transpose()?,
            payload: self
                .payload
                .map(|callback| retain(state, &callback))
                .transpose()?,
            response: self
                .response
                .map(|callback| retain(state, &callback))
                .transpose()?,
            objects: self
                .objects
                .map(|object| crate::callback_imports::retain_object(state, &object))
                .transpose()?,
        })
    }
}
/// Install the original assistant-buffer resource and synchronous setup boundary.
pub(crate) fn add_to_linker<T: CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    crate::provider_imports::add_to_linker(host)?;
    host.func_wrap(
        "model-stream",
        |mut s: StoreContextMut<'_, T>, (simple, data, controls): (bool, String, WireControls)| {
            let controls = controls.retained(s.data_mut())?;
            let stream = s.data_mut().api().model_stream(simple, data, controls);
            let stream = match stream {
                Ok(stream) => Ok(s.data_mut().ctx().table.push(stream)?),
                Err(error) => Err(error),
            };
            Ok((stream,))
        },
    )?;
    host.func_wrap_concurrent(
        "[method]assistant-stream.next",
        |a, (stream,): (Resource<Endpoint>,)| {
            Box::pin(async move {
                let stream = a.with(|mut access| access.get().ctx().table.get(&stream).cloned())?;
                Ok((stream.next().await,))
            })
        },
    )?;
    host.func_wrap_concurrent(
        "[method]assistant-stream.result",
        |a, (stream,): (Resource<Endpoint>,)| {
            Box::pin(async move {
                let stream = a.with(|mut access| access.get().ctx().table.get(&stream).cloned())?;
                Ok((stream.result().await,))
            })
        },
    )?;
    Ok(())
}

impl HostAssistantStream for State {
    fn push(
        &mut self,
        this: Resource<Endpoint>,
        event: String,
    ) -> wasmtime::Result<Result<(), String>> {
        Ok(endpoint(self, &this)?.push(event))
    }

    fn end(
        &mut self,
        this: Resource<Endpoint>,
        value: Option<String>,
    ) -> wasmtime::Result<Result<(), String>> {
        Ok(endpoint(self, &this)?.end(value))
    }

    fn drop(&mut self, this: Resource<Endpoint>) -> wasmtime::Result<()> {
        release(self, this)
    }
}

/// Forwards the synchronous methods of an assistant stream to its trait implementation.
pub(crate) fn add_stream(host: &mut LinkerInstance<'_, State>) -> wasmtime::Result<()> {
    add_resource::<State, Endpoint>(host, "assistant-stream")?;
    forwards! { host, "assistant-stream", HostAssistantStream, Endpoint,
        "push" => push(event: String);
        "end" => end(value: Option<String>);
    }
    Ok(())
}
