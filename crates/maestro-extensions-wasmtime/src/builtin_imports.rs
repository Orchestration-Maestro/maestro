//! Canonical resources returned by the real native built-in factories.
use crate::{
    builtin_reads::{Endpoint, Execution, Request},
    callback_imports::CallbackView,
    local_resources::SignalEndpoint,
};
use wasmtime::{
    StoreContextMut,
    component::{LinkerInstance, Resource},
};

/// Installs factory, metadata, preparation and asynchronous execution on one native owner.
pub(crate) fn add_to_linker<T: CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    crate::context_imports::add_resource::<T, Endpoint>(host, "builtin-tool")?;
    super::mutation_imports::add_to_linker(host)?;
    super::local_bash_imports::add_to_linker(host)?;
    add_bash_factory(host)?;
    super::builtin_selection_imports::add_to_linker(host)?;
    add_factories(host)?;
    crate::read_imports::add_read(
        host,
        "[method]builtin-tool.metadata",
        Request::Metadata,
        crate::read_imports::resource::<T, Endpoint>,
    )?;
    crate::read_imports::add_operand(
        host,
        "[method]builtin-tool.prepare",
        Request::Prepare,
        crate::read_imports::operand::<T, Endpoint, String>,
    )?;
    host.func_wrap_concurrent(
        "[method]builtin-tool.execute",
        |accessor, (tool, id, args, model, signal, update): ExecutionArgs| {
            Box::pin(async move {
                let (tool, signal, update, api) = accessor.with(|mut access| {
                    let state = access.get();
                    Ok::<_, wasmtime::Error>((
                        state.ctx().table.get(&tool)?.clone(),
                        signal
                            .as_ref()
                            .map(|signal| state.ctx().table.get(signal).cloned())
                            .transpose()?,
                        update
                            .as_ref()
                            .map(|update| crate::callback_imports::retain_object(state, update))
                            .transpose()?,
                        state.api().clone(),
                    ))
                })?;
                Ok((tool
                    .execute(Execution {
                        id,
                        args,
                        model,
                        signal,
                        update,
                        api,
                    })
                    .await,))
            })
        },
    )?;
    Ok(())
}
/// Installs native factory returns without copying their typed definitions.
fn add_factories<T: CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    host.func_wrap(
        "create-read-tool",
        |mut store: StoreContextMut<'_, T>,
         (cwd, resize, operations): (
            String,
            Option<bool>,
            Option<Resource<crate::guest_calls::Callback>>,
        )| {
            let operations = operations
                .as_ref()
                .map(|operations| {
                    crate::callback_imports::retain_object(store.data_mut(), operations)
                })
                .transpose()?;
            let result = store.data().api().create_read(cwd, resize, operations);
            let result = match result {
                Ok(tool) => Ok(store.data_mut().ctx().table.push(tool)?),
                Err(error) => Err(error),
            };
            Ok((result,))
        },
    )?;
    add_mutating_factories(host)
}

/// Installs constructors sharing the optional retained-effect signature.
fn add_mutating_factories<T: CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    for (name, create) in [
        (
            "create-ls-tool",
            crate::api_reads::Endpoint::create_ls as MutationFactory,
        ),
        (
            "create-grep-tool",
            crate::api_reads::Endpoint::create_grep as MutationFactory,
        ),
        (
            "create-find-tool",
            crate::api_reads::Endpoint::create_find as MutationFactory,
        ),
        (
            "create-write-tool",
            crate::api_reads::Endpoint::create_write as MutationFactory,
        ),
        (
            "create-edit-tool",
            crate::api_reads::Endpoint::create_edit as MutationFactory,
        ),
    ] {
        let factory = move |mut store: StoreContextMut<'_, T>,
                            (cwd, operations): (
            String,
            Option<Resource<crate::guest_calls::Callback>>,
        )| {
            let operations = operations
                .as_ref()
                .map(|operations| {
                    crate::callback_imports::retain_object(store.data_mut(), operations)
                })
                .transpose()?;
            let result = create(store.data().api(), cwd, operations);
            let result = match result {
                Ok(tool) => Ok(store.data_mut().ctx().table.push(tool)?),
                Err(error) => Err(error),
            };
            Ok((result,))
        };
        host.func_wrap(name, factory)?;
    }
    Ok(())
}
/// Execution keeps its native tool and cancellation as separate typed resources.
type ExecutionArgs = (
    Resource<Endpoint>,
    String,
    String,
    Option<String>,
    Option<Resource<SignalEndpoint>>,
    Option<Resource<crate::guest_calls::Callback>>,
);

/// Constructor signature shared by factories with optional replacement effects.
type MutationFactory = fn(
    &crate::api_reads::Endpoint,
    String,
    Option<crate::guest_calls::Handle>,
) -> Result<Endpoint, String>;

/// Admits both optional shell callbacks without invoking them during construction.
fn add_bash_factory<T: CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    host.func_wrap(
        "create-bash-tool",
        |mut store: StoreContextMut<'_, T>, (cwd, controls, operations, hook): BashFactoryArgs| {
            let mut retain = |callback: Option<Resource<crate::guest_calls::Callback>>| {
                callback
                    .as_ref()
                    .map(|callback| {
                        crate::callback_imports::retain_object(store.data_mut(), callback)
                    })
                    .transpose()
            };
            let options = crate::local_resources::builtin_bash::Options {
                controls,
                operations: retain(operations)?,
                hook: retain(hook)?,
            };
            let result = store.data().api().create_bash(cwd, options);
            let result = match result {
                Ok(tool) => Ok(store.data_mut().ctx().table.push(tool)?),
                Err(error) => Err(error),
            };
            Ok((result,))
        },
    )
}

/// Shell controls travel beside independently typed callback capabilities.
type BashFactoryArgs = (
    String,
    crate::local_resources::builtin_bash::Controls,
    Option<Resource<crate::guest_calls::Callback>>,
    Option<Resource<crate::guest_calls::Callback>>,
);
