//! Canonical registrations retain actual command, shortcut and tool callbacks.
use crate::{
    callback_imports::{self, CallbackView},
    guest_calls::Callback,
    registration_reads::Request,
};
use wasmtime::{
    StoreContextMut,
    component::{Accessor, LinkerInstance, Resource},
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
    host.func_wrap_concurrent("register-tool", |accessor, args| {
        Box::pin(register_tool(accessor, args))
    })?;
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

/// Describes guest schema callbacks before routing the native tool refresh.
async fn register_tool<T: CallbackView + 'static>(
    accessor: &Accessor<T>,
    (metadata, prepare, execute, schemas, render_call, render_result): Registration,
) -> wasmtime::Result<(Resource<crate::import_route::Route>,)> {
    let exports = accessor
        .with(|mut access| access.get().exports())
        .map_err(wasmtime::Error::msg)?;
    let mut described = Vec::new();
    for (pointer, handler) in schemas {
        let shape = match exports
            .schema
            .describe(accessor, Resource::new_borrow(handler.rep()))
            .await?
        {
            Ok(shape) => shape,
            Err(error) => {
                let route = crate::import_route::Route::failed(error.message);
                return accessor
                    .with(|mut access| Ok((crate::import_route::push(access.get(), route)?,)));
            }
        };
        let handle =
            accessor.with(|mut access| callback_imports::retain(access.get(), &handler))?;
        described.push((pointer, shape, handle));
    }
    accessor.with(|mut access| {
        let store = access.get();
        let prepare = callback_imports::optional(store, prepare.as_ref())?;
        let execute = callback_imports::retain(store, &execute)?;
        let renderers = (
            callback_imports::optional(store, render_call.as_ref())?,
            callback_imports::optional(store, render_result.as_ref())?,
        );
        let route = store.api().routed_registration(|reply| Request::Tool {
            metadata,
            prepare,
            execute,
            schemas: described,
            renderers,
            reply,
        });
        Ok((crate::import_route::push(store, route)?,))
    })
}

/// Registration's canonical callback operands.
type Registration = (
    String,
    Option<Resource<Callback>>,
    Resource<Callback>,
    Vec<(String, Resource<Callback>)>,
    Option<Resource<Callback>>,
    Option<Resource<Callback>>,
);
