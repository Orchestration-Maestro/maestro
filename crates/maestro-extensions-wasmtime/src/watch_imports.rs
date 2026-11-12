//! Canonical file watches retain their original native owner and callback identity.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{
    bindings::maestro::extension::host::HostFsWatcher,
    callback_imports::{CallbackView, retain},
    context_imports::add_resource,
    guest_calls::Callback,
    lifecycle::State,
    local_resources::{
        Message,
        watches::{Request, Watcher},
    },
    read_imports::{endpoint, forwards, release},
};
use wasmtime::{
    StoreContextMut,
    component::{LinkerInstance, Resource},
};

/// Installs watch registration through the existing local request pump.
pub(crate) fn add_to_linker<T: CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    host.func_wrap(
        "watch",
        |mut store: StoreContextMut<'_, T>, (path, listener): (String, Resource<Callback>)| {
            let listener = retain(store.data_mut(), &listener)?;
            let result = store
                .data()
                .api()
                .0
                .request(|id, reply| Message::Watch(id, Request::Open(path, listener, reply)));
            let result = match result {
                Ok(watch) => Ok(store.data_mut().ctx().table.push(watch)?),
                Err(error) => Err(error),
            };
            Ok((result,))
        },
    )
}

impl HostFsWatcher for State {
    fn close(&mut self, this: Resource<Watcher>) -> wasmtime::Result<Result<(), String>> {
        Ok(endpoint(self, &this)?.close())
    }

    fn drop(&mut self, this: Resource<Watcher>) -> wasmtime::Result<()> {
        release(self, this)
    }
}

/// Forwards the close method of a watch registration to its trait implementation.
pub(crate) fn add_watcher(host: &mut LinkerInstance<'_, State>) -> wasmtime::Result<()> {
    add_resource::<State, Watcher>(host, "fs-watcher")?;
    forwards! { host, "fs-watcher", HostFsWatcher, Watcher,
        "close" => close();
    }
    Ok(())
}
