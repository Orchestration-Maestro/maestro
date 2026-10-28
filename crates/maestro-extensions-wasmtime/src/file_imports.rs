//! Typed file operands cross the existing pump to the application owner.
use crate::api_imports::ApiView;
use crate::local_resources::{Message, native_resources::Request};
use wasmtime::{StoreContextMut, component::LinkerInstance};

/// Installs typed imports without a second filesystem implementation.
pub(crate) fn add_to_linker<T: ApiView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    host.func_wrap(
        "save-file",
        |store: StoreContextMut<'_, T>, (path, bytes, mode): (String, Vec<u8>, u32)| {
            Ok((store.data().api().0.request(|id, reply| {
                Message::Native(id, Request::SaveFile(path, bytes, mode, reply))
            }),))
        },
    )?;
    host.func_wrap(
        "file-mode",
        |store: StoreContextMut<'_, T>, (path,): (String,)| {
            Ok((store
                .data()
                .api()
                .0
                .request(|id, reply| Message::Native(id, Request::FileMode(path, reply))),))
        },
    )?;
    host.func_wrap(
        "set-file-mode",
        |store: StoreContextMut<'_, T>, (path, mode): (String, u32)| {
            Ok((store
                .data()
                .api()
                .0
                .request(|id, reply| Message::Native(id, Request::SetFileMode(path, mode, reply))),))
        },
    )?;
    host.func_wrap(
        "create-dir",
        |store: StoreContextMut<'_, T>, (path, mode): (String, u32)| {
            Ok((store
                .data()
                .api()
                .0
                .request(|id, reply| Message::Native(id, Request::CreateDir(path, mode, reply))),))
        },
    )
}
