//! Canonical file watches retain their original native owner and callback identity.
use crate::{
    callback_imports::{CallbackView, retain},
    guest_calls::Callback,
    local_resources::{
        Message,
        watches::{Request, Watcher},
    },
};
use wasmtime::{
    StoreContextMut,
    component::{LinkerInstance, Resource},
};

/// Installs watch registration and close through the existing local request pump.
pub(crate) fn add_to_linker<T: CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    crate::context_imports::add_resource::<T, Watcher>(host, "fs-watcher")?;
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
    )?;
    host.func_wrap(
        "[method]fs-watcher.close",
        |mut store: StoreContextMut<'_, T>, (watch,): (Resource<Watcher>,)| {
            let watch = store.data_mut().ctx().table.get(&watch)?.clone();
            Ok((watch
                .0
                .request(|id, reply| Message::Watch(id, Request::Close(reply))),))
        },
    )
}
