//! Canonical registrations retain actual command, shortcut and tool callbacks.
use crate::{
    callback_imports::{self, CallbackView},
    guest_calls::Callback,
    registration_reads::Request,
};
use wasmtime::{
    StoreContextMut,
    component::{LinkerInstance, Resource},
};

/// Complete command registration operands without collapsing absent callbacks.
type CommandArgs = (
    String,
    Option<String>,
    Option<Resource<Callback>>,
    Resource<Callback>,
);

/// Installs callback-bearing registration on the original native API endpoint.
pub(crate) fn add_to_linker<T: CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    host.func_wrap(
        "on",
        |mut store: StoreContextMut<'_, T>, (name, handler): (String, Resource<Callback>)| {
            let handler = callback_imports::retain(store.data_mut(), &handler)?;
            Ok((store
                .data()
                .api()
                .registration(|reply| Request::On(name, handler, reply)),))
        },
    )?;
    host.func_wrap(
        "register-message-renderer",
        |mut store: StoreContextMut<'_, T>, (name, handler): (String, Resource<Callback>)| {
            let handler = callback_imports::retain(store.data_mut(), &handler)?;
            Ok((store
                .data()
                .api()
                .registration(|reply| Request::Renderer(name, handler, reply)),))
        },
    )?;
    add_tool(host)?;
    host.func_wrap(
        "register-command",
        |mut store: StoreContextMut<'_, T>,
         (name, description, completions, handler): CommandArgs| {
            let completions = callback_imports::optional(store.data_mut(), completions.as_ref())?;
            let handler = callback_imports::retain(store.data_mut(), &handler)?;
            Ok((store.data().api().registration(|reply| Request::Command {
                name,
                description,
                completions,
                handler,
                reply,
            }),))
        },
    )?;
    host.func_wrap(
        "register-shortcut",
        |mut store: StoreContextMut<'_, T>,
         (key, description, handler): (String, Option<String>, Resource<Callback>)| {
            let handler = callback_imports::retain(store.data_mut(), &handler)?;
            Ok((store.data().api().registration(|reply| Request::Shortcut {
                key,
                description,
                handler,
                reply,
            }),))
        },
    )?;
    Ok(())
}

/// Installs tool registration, whose native tool refresh may call back into this instance.
fn add_tool<T: CallbackView + 'static>(host: &mut LinkerInstance<'_, T>) -> wasmtime::Result<()> {
    host.func_wrap(
        "register-tool",
        |mut store: StoreContextMut<'_, T>,
         (metadata, prepare, execute): (String, Option<Resource<Callback>>, Resource<Callback>)| {
            let prepare = callback_imports::optional(store.data_mut(), prepare.as_ref())?;
            let execute = callback_imports::retain(store.data_mut(), &execute)?;
            let route = store.data().api().routed_registration(|reply| Request::Tool {
                metadata,
                prepare,
                execute,
                reply,
            });
            Ok((crate::import_route::push(store.data_mut(), route)?,))
        },
    )?;
    Ok(())
}
