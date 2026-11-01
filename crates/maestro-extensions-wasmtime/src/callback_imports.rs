//! Canonical callback identities retain their original Store admission.
use crate::{
    api_imports::ApiView,
    guest_calls::{Callback, Callbacks, Handle},
};
use wasmtime::{
    StoreContextMut,
    component::{LinkerInstance, Resource},
};

/// Store-owned weak callback service, separate from local domain ownership.
pub(crate) trait CallbackView: ApiView {
    /// Returns callback identities associated with this Store alone.
    fn callbacks(&mut self) -> &mut Callbacks;
    /// Read the canonical exports resolved for this Store.
    fn exports(&self) -> Result<std::sync::Arc<crate::guest_exports::Exports>, String>;
}

/// Installs identity allocation and lookup without inventing a closure owner.
pub(crate) fn add_to_linker<T: CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    crate::context_imports::add_resource::<T, Callback>(host, "callback")?;
    host.func_wrap(
        "[constructor]callback",
        |mut store: StoreContextMut<'_, T>, (): ()| {
            let callback = store
                .data_mut()
                .callbacks()
                .allocate()
                .map_err(wasmtime::Error::msg)?;
            Ok((store.data_mut().ctx().table.push(callback)?,))
        },
    )?;
    host.func_wrap(
        "[method]callback.id",
        |mut store: StoreContextMut<'_, T>, (callback,): (Resource<Callback>,)| {
            Ok((store.data_mut().ctx().table.get(&callback)?.id,))
        },
    )?;
    Ok(())
}

/// Resolves the borrowed canonical identity before releasing Store access.
pub(crate) fn retain<T: CallbackView>(
    store: &mut T,
    callback: &Resource<Callback>,
) -> wasmtime::Result<Handle> {
    let id = store.ctx().table.get(callback)?.id;
    let admission = store.api().admission();
    store
        .callbacks()
        .retain(id, admission)
        .map_err(wasmtime::Error::msg)
}

/// Takes ownership of an authored object the guest returned under a fresh identity.
pub(crate) fn transferred<T: CallbackView>(
    store: &mut T,
    object: Resource<Callback>,
) -> wasmtime::Result<Handle> {
    let id = store.ctx().table.delete(object)?.id;
    let admission = store.api().admission();
    store
        .callbacks()
        .retain_object(id, admission)
        .map_err(wasmtime::Error::msg)
}

/// Preserves an absent callback rather than manufacturing an empty closure.
pub(crate) fn optional<T: CallbackView>(
    store: &mut T,
    callback: Option<&Resource<Callback>>,
) -> wasmtime::Result<Option<Handle>> {
    callback.map(|callback| retain(store, callback)).transpose()
}

/// Retain a borrowed authored value with destruction deferred until Store quiescence.
pub(crate) fn retain_object<T: CallbackView>(
    store: &mut T,
    object: &Resource<Callback>,
) -> wasmtime::Result<Handle> {
    let id = store.ctx().table.get(object)?.id;
    let admission = store.api().admission();
    store
        .callbacks()
        .retain_object(id, admission)
        .map_err(wasmtime::Error::msg)
}
