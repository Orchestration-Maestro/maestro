//! Canonical event imports retain native listeners and invocation frames.
use crate::{
    callback_imports::{self, CallbackView},
    event_reads::Request,
    guest_calls::Callback,
    local_resources::effects::EmitterEndpoint,
};
use wasmtime::{
    StoreContextMut,
    component::{LinkerInstance, Resource},
};

/// Installs ordinary and callback-scoped emission on the sole local request pump.
pub(crate) fn add_to_linker<T: CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    host.func_wrap(
        "event-on",
        |mut store: StoreContextMut<'_, T>, (channel, handler): (String, Resource<Callback>)| {
            let callback = callback_imports::retain(store.data_mut(), &handler)?;
            let result = store
                .data()
                .api()
                .event(|reply| Request::On(channel, callback, reply));
            let result = match result {
                Ok(subscription) => Ok(store.data_mut().ctx().table.push(subscription)?),
                Err(error) => Err(error),
            };
            Ok((result,))
        },
    )?;
    host.func_wrap_concurrent(
        "event-emit",
        |accessor, (channel, data): (String, String)| {
            Box::pin(async move {
                let api = accessor.with(|mut access| access.get().api().clone());
                Ok((api.emit(channel, data).await,))
            })
        },
    )?;
    add_emission_to_linker(host)?;
    host.func_wrap(
        "start-event-tail",
        |mut store: StoreContextMut<'_, T>,
         (resource, tail): (Resource<EmitterEndpoint>, Resource<Callback>)| {
            let tail = callback_imports::retain(store.data_mut(), &tail)?;
            let emitter = store.data_mut().ctx().table.get(&resource)?.clone();
            Ok((emitter.start_tail(tail),))
        },
    )?;
    Ok(())
}

/// Installs the synchronous snapshot walk without host reentry into an active instance.
fn add_emission_to_linker<T: CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    add_local_prefix_to_linker(host)?;
    host.func_wrap(
        "callback-emit",
        |mut store: StoreContextMut<'_, T>,
         (resource, channel, data): (Resource<EmitterEndpoint>, String, String)| {
            Ok((store
                .data_mut()
                .ctx()
                .table
                .get(&resource)?
                .snapshot(channel, data),))
        },
    )?;
    host.func_wrap(
        "callback-segment",
        |mut store: StoreContextMut<'_, T>,
         (resource, snapshot, start, end): (Resource<EmitterEndpoint>, u32, u32, u32)| {
            let emitter = store.data_mut().ctx().table.get(&resource)?.clone();
            let route = emitter.segment(snapshot, start, end);
            Ok((crate::import_route::push(store.data_mut(), route)?,))
        },
    )?;
    host.func_wrap(
        "callback-end-snapshot",
        |mut store: StoreContextMut<'_, T>,
         (resource, snapshot): (Resource<EmitterEndpoint>, u32)| {
            Ok((store
                .data_mut()
                .ctx()
                .table
                .get(&resource)?
                .end_snapshot(snapshot),))
        },
    )?;
    Ok(())
}

/// Installs the independent return boundary of guest-local listener prefixes.
fn add_local_prefix_to_linker<T: CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    host.func_wrap(
        "callback-begin-local",
        |mut store: StoreContextMut<'_, T>,
         (resource, snapshot, index): (Resource<EmitterEndpoint>, u32, u32)| {
            let result = store
                .data_mut()
                .ctx()
                .table
                .get(&resource)?
                .begin_local(snapshot, index);
            let result = match result {
                Ok(endpoint) => Ok(store.data_mut().ctx().table.push(endpoint)?),
                Err(error) => Err(error),
            };
            Ok((result,))
        },
    )?;
    host.func_wrap(
        "callback-end-local",
        |mut store: StoreContextMut<'_, T>,
         (resource, error): (Resource<EmitterEndpoint>, Option<String>)| {
            let emitter = store.data_mut().ctx().table.get(&resource)?.clone();
            let route = emitter.end_local(error);
            Ok((crate::import_route::push(store.data_mut(), route)?,))
        },
    )?;
    Ok(())
}
