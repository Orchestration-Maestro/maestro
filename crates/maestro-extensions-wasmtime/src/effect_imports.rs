//! Canonical progress and subscription operations retain their native owners.
use crate::{
    context_imports::add_resource,
    local_resources::{
        bash_output::Endpoint as BashOutput,
        effects::{EmitterEndpoint, SubscriptionEndpoint, ThemeEndpoint, ToolUpdate},
    },
};
use wasmtime::{
    StoreContextMut,
    component::{LinkerInstance, Resource},
};
use wasmtime_wasi::WasiView;

/// Installs real operand resources without constructing a substitute observer or bus.
pub(crate) fn add_to_linker<T: WasiView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    add_resource::<T, BashOutput>(host, "bash-output")?;
    add_resource::<T, ToolUpdate>(host, "tool-update")?;
    add_resource::<T, SubscriptionEndpoint>(host, "subscription")?;
    add_resource::<T, ThemeEndpoint>(host, "theme")?;
    add_resource::<T, EmitterEndpoint>(host, "callback-emitter")?;
    host.func_wrap(
        "[method]bash-output.on-data",
        |mut store: StoreContextMut<'_, T>, (resource, bytes): (Resource<BashOutput>, Vec<u8>)| {
            let output = store.data_mut().ctx().table.get(&resource)?.clone();
            Ok((output.on_data(bytes),))
        },
    )?;
    host.func_wrap(
        "[method]tool-update.update",
        |mut store: StoreContextMut<'_, T>, (resource, partial): (Resource<ToolUpdate>, String)| {
            let update = store.data_mut().ctx().table.get(&resource)?.clone();
            Ok((crate::import_route::push(
                store.data_mut(),
                update.update(partial),
            )?,))
        },
    )?;
    host.func_wrap(
        "[method]subscription.unsubscribe",
        |mut store: StoreContextMut<'_, T>, (resource,): (Resource<SubscriptionEndpoint>,)| {
            let subscription = store.data_mut().ctx().table.get(&resource)?.clone();
            Ok((subscription.unsubscribe(),))
        },
    )?;
    Ok(())
}
