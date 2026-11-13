//! Canonical typed stream resources backed by the original native buffer.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{
    bindings::maestro::extension::host::HostEventStream,
    callback_imports::{CallbackView, transferred},
    context_imports::add_resource,
    guest_calls::{Callback, Handle},
    import_route::{Route, push},
    lifecycle::State,
    local_resources::stream_reads::Endpoint,
    read_imports::{endpoint, forwards, release},
};
use wasmtime::{
    StoreContextMut,
    component::{Accessor, LinkerInstance, Resource},
};
use wasmtime_wasi::WasiView as _;

/// Install typed resources and their existing synchronous callback routes.
pub(crate) fn add_to_linker<T: CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    crate::context_imports::add_resource::<T, Handle>(host, "stream-value")?;
    host.func_wrap(
        "[constructor]stream-value",
        |mut s: StoreContextMut<'_, T>, (value,): (Resource<Callback>,)| {
            let value = transferred(s.data_mut(), value)?;
            Ok((s.data_mut().ctx().table.push(value)?,))
        },
    )?;
    host.func_wrap(
        "[method]stream-value.id",
        |mut s: StoreContextMut<'_, T>, (value,): (Resource<Handle>,)| {
            Ok((s.data_mut().ctx().table.get(&value)?.id(),))
        },
    )?;
    host.func_wrap(
        "new-event-stream",
        |mut s: StoreContextMut<'_, T>,
         (classifier, extractor): (Resource<Callback>, Resource<Callback>)| {
            let classifier = crate::callback_imports::retain(s.data_mut(), &classifier)?;
            let extractor = crate::callback_imports::retain(s.data_mut(), &extractor)?;
            let stream = s.data_mut().api().new_stream(classifier, extractor);
            let stream = match stream {
                Ok(stream) => Ok(s.data_mut().ctx().table.push(stream)?),
                Err(error) => Err(error),
            };
            Ok((stream,))
        },
    )?;
    host.func_wrap_concurrent(
        "[method]event-stream.next",
        |a, (stream,): (Resource<Endpoint>,)| Box::pin(next(a, stream)),
    )?;
    host.func_wrap_concurrent(
        "[method]event-stream.result",
        |a, (stream,): (Resource<Endpoint>,)| Box::pin(result(a, stream)),
    )?;
    Ok(())
}
impl HostEventStream for State {
    fn push(
        &mut self,
        this: Resource<Endpoint>,
        value: Resource<Handle>,
    ) -> wasmtime::Result<Resource<Route>> {
        let stream = endpoint(self, &this)?;
        let value = self.ctx().table.delete(value)?;
        push(self, stream.push(value))
    }

    fn end(
        &mut self,
        this: Resource<Endpoint>,
        value: Option<Resource<Handle>>,
    ) -> wasmtime::Result<Resource<Route>> {
        let stream = endpoint(self, &this)?;
        let value = value
            .map(|value| self.ctx().table.delete(value))
            .transpose()?;
        push(self, stream.end(value))
    }

    fn drop(&mut self, this: Resource<Endpoint>) -> wasmtime::Result<()> {
        release(self, this)
    }
}

/// Forwards the synchronous methods of an event stream to its trait implementation.
pub(crate) fn add_event_stream(host: &mut LinkerInstance<'_, State>) -> wasmtime::Result<()> {
    add_resource::<State, Endpoint>(host, "event-stream")?;
    forwards! { host, "event-stream", HostEventStream, Endpoint,
        "push" => push(value: Resource<Handle>);
        "end" => end(value: Option<Resource<Handle>>);
    }
    Ok(())
}

/// Await a competing reader with no Store borrow held.
async fn next<T: CallbackView + 'static>(
    a: &Accessor<T>,
    stream: Resource<Endpoint>,
) -> wasmtime::Result<(Result<Option<Resource<Handle>>, String>,)> {
    let stream = a.with(|mut access| access.get().ctx().table.get(&stream).cloned())?;
    let value = stream.next().await;
    let value = match value {
        Ok(value) => Ok(value
            .map(|value| a.with(|mut access| access.get().ctx().table.push(value)))
            .transpose()?),
        Err(error) => Err(error),
    };
    Ok((value,))
}
/// Await the retained result without draining events or holding Store access.
async fn result<T: CallbackView + 'static>(
    a: &Accessor<T>,
    stream: Resource<Endpoint>,
) -> wasmtime::Result<(Result<Resource<Handle>, String>,)> {
    let stream = a.with(|mut access| access.get().ctx().table.get(&stream).cloned())?;
    let value = stream.result().await;
    let value = match value {
        Ok(value) => Ok(a.with(|mut access| access.get().ctx().table.push(value))?),
        Err(error) => Err(error),
    };
    Ok((value,))
}
