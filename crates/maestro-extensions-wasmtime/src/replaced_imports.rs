//! Canonical continuation methods preserve the creating session's native operations.
use crate::{
    context_imports::{CommandContext, add_resource},
    replaced_reads::{Endpoint, Request},
};
use wasmtime::{
    StoreContextMut,
    component::{LinkerInstance, Resource},
};
use wasmtime_wasi::WasiView;

/// Defines the independently retained continuation context and its real message routes.
pub(crate) fn add_to_linker<T: WasiView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    add_resource::<T, Endpoint>(host, "replaced-session-context")?;
    host.func_wrap(
        "[method]replaced-session-context.command",
        |mut store: StoreContextMut<'_, T>, (resource,): (Resource<Endpoint>,)| {
            let endpoint = store.data_mut().ctx().table.get(&resource)?.clone();
            let command = endpoint.command().map_err(wasmtime::Error::msg)?;
            Ok((store.data_mut().ctx().table.push(CommandContext(command))?,))
        },
    )?;
    for (method, request) in [
        ("send-message", Request::Message as fn(_, _, _) -> _),
        ("send-user-message", Request::UserMessage),
    ] {
        host.func_wrap_concurrent(&format!("[method]replaced-session-context.{method}"), move |accessor, (resource, value, options): (Resource<Endpoint>, String, Option<String>)| {
            Box::pin(async move {
                let endpoint = accessor.with(|mut access| Ok::<_, wasmtime::Error>(access.get().ctx().table.get(&resource)?.clone()))?;
                Ok((endpoint.send(|reply| request(value, options, reply)).await,))
            })
        })?;
    }
    Ok(())
}
