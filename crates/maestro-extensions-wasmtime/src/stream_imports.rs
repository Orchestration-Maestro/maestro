//! Canonical typed stream resources backed by the original native buffer.
use crate::{
    callback_imports::{CallbackView, transferred},
    guest_calls::{Callback, Handle},
    local_resources::stream_reads::Endpoint,
};
use wasmtime::{
    StoreContextMut,
    component::{Accessor, LinkerInstance, Resource},
};

/// Install typed resources and their existing synchronous callback routes.
pub(crate) fn add_to_linker<T: CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    crate::context_imports::add_resource::<T, Handle>(host, "stream-value")?;
    crate::context_imports::add_resource::<T, Endpoint>(host, "event-stream")?;
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
    host.func_wrap(
        "[method]event-stream.push",
        |mut s: StoreContextMut<'_, T>, (stream, value): (Resource<Endpoint>, Resource<Handle>)| {
            let table = s.data_mut().ctx().table;
            let value = table.delete(value)?;
            let route = table.get(&stream)?.push(value);
            Ok((table.push(route)?,))
        },
    )?;
    host.func_wrap(
        "[method]event-stream.end",
        |mut s: StoreContextMut<'_, T>,
         (stream, value): (Resource<Endpoint>, Option<Resource<Handle>>)| {
            let table = s.data_mut().ctx().table;
            let value = value.map(|value| table.delete(value)).transpose()?;
            let route = table.get(&stream)?.end(value);
            Ok((table.push(route)?,))
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
