//! Canonical command-context imports: the generated store-taking trait over the original local
//! command owner, with callback operands retained before the call crosses to it.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{
    bindings::maestro::extension::host::{
        HostCommandContextWithStore, NewSessionCommandData, SessionChangeResult,
        SwitchSessionOptions,
    },
    callback_imports,
    context_imports::CommandContext,
    context_reads::Endpoint,
    guest_calls::Callback,
    lifecycle::State,
    session_calls::{NewSession, Switch},
};
use wasmtime::component::{Accessor, HasSelf, LinkerInstance, Resource};
use wasmtime_wasi::WasiView;

/// Complete owned operands of the canonical new-session import.
type NewSessionArgs = (
    Resource<CommandContext>,
    Option<NewSessionCommandData>,
    Option<Resource<Callback>>,
    Option<Resource<Callback>>,
);

impl From<maestro_extensions::types::SessionChangeResult> for SessionChangeResult {
    fn from(result: maestro_extensions::types::SessionChangeResult) -> Self {
        Self {
            cancelled: result.cancelled,
        }
    }
}

/// Releases Store access before awaiting the original local command operation.
fn endpoint(
    accessor: &Accessor<State, HasSelf<State>>,
    this: &Resource<CommandContext>,
) -> wasmtime::Result<Endpoint> {
    accessor.with(|mut access| Ok(access.get().ctx().table.get(this)?.0.clone()))
}

impl HostCommandContextWithStore<State> for HasSelf<State> {
    async fn wait_for_idle(
        accessor: &Accessor<State, Self>,
        this: Resource<CommandContext>,
    ) -> wasmtime::Result<Result<(), String>> {
        Ok(endpoint(accessor, &this)?.wait_for_idle().await)
    }

    async fn new_session(
        accessor: &Accessor<State, Self>,
        this: Resource<CommandContext>,
        data: Option<NewSessionCommandData>,
        setup: Option<Resource<Callback>>,
        with_session: Option<Resource<Callback>>,
    ) -> wasmtime::Result<Result<SessionChangeResult, String>> {
        let context = endpoint(accessor, &this)?;
        let options = accessor.with(|mut access| {
            let store = access.get();
            Ok::<_, wasmtime::Error>(NewSession {
                data,
                setup: callback_imports::optional(store, setup.as_ref())?,
                with_session: callback_imports::optional(store, with_session.as_ref())?,
            })
        })?;
        Ok(context
            .new_session(options)
            .await
            .map(SessionChangeResult::from))
    }

    async fn fork(
        accessor: &Accessor<State, Self>,
        this: Resource<CommandContext>,
        entry_id: String,
        data: Option<String>,
        with_session: Option<Resource<Callback>>,
    ) -> wasmtime::Result<Result<SessionChangeResult, String>> {
        let context = endpoint(accessor, &this)?;
        let callback = accessor
            .with(|mut access| callback_imports::optional(access.get(), with_session.as_ref()))?;
        Ok(context
            .fork(entry_id, data, callback)
            .await
            .map(SessionChangeResult::from))
    }

    async fn navigate_tree(
        accessor: &Accessor<State, Self>,
        this: Resource<CommandContext>,
        target_id: String,
        options: Option<String>,
    ) -> wasmtime::Result<Result<SessionChangeResult, String>> {
        let context = endpoint(accessor, &this)?;
        let options = options
            .as_deref()
            .map(crate::writer_reads::decode)
            .transpose();
        Ok(match options {
            Ok(options) => context
                .navigate_tree(target_id, options)
                .await
                .map(SessionChangeResult::from),
            Err(error) => Err(error),
        })
    }

    async fn switch_session(
        accessor: &Accessor<State, Self>,
        this: Resource<CommandContext>,
        path: String,
        options: Option<SwitchSessionOptions>,
    ) -> wasmtime::Result<Result<SessionChangeResult, String>> {
        let context = endpoint(accessor, &this)?;
        let options = accessor.with(|mut access| {
            options
                .map(|options| {
                    callback_imports::optional(access.get(), options.with_session.as_ref())
                        .map(|with_session| Switch { with_session })
                })
                .transpose()
        })?;
        Ok(context
            .switch_session(path, options)
            .await
            .map(SessionChangeResult::from))
    }

    async fn reload(
        accessor: &Accessor<State, Self>,
        this: Resource<CommandContext>,
    ) -> wasmtime::Result<Result<(), String>> {
        Ok(endpoint(accessor, &this)?.reload().await)
    }
}

/// Forwards each concurrent command-context method to the generated trait, one line each.
pub(crate) fn add_to_linker(host: &mut LinkerInstance<'_, State>) -> wasmtime::Result<()> {
    type Imports = HasSelf<State>;
    host.func_wrap_concurrent(
        "[method]command-context.wait-for-idle",
        |accessor, (this,): (Resource<CommandContext>,)| {
            Box::pin(async move { Ok((Imports::wait_for_idle(accessor, this).await?,)) })
        },
    )?;
    host.func_wrap_concurrent(
        "[method]command-context.reload",
        |accessor, (this,): (Resource<CommandContext>,)| {
            Box::pin(async move { Ok((Imports::reload(accessor, this).await?,)) })
        },
    )?;
    host.func_wrap_concurrent(
        "[method]command-context.navigate-tree",
        |accessor, (this, id, options): (Resource<CommandContext>, String, Option<String>)| {
            Box::pin(
                async move { Ok((Imports::navigate_tree(accessor, this, id, options).await?,)) },
            )
        },
    )?;
    host.func_wrap_concurrent(
        "[method]command-context.new-session",
        |accessor, (this, data, setup, with_session): NewSessionArgs| {
            Box::pin(async move {
                Ok((Imports::new_session(accessor, this, data, setup, with_session).await?,))
            })
        },
    )?;
    host.func_wrap_concurrent(
        "[method]command-context.fork",
        |accessor,
         (this, id, data, callback): (
            Resource<CommandContext>,
            String,
            Option<String>,
            Option<Resource<Callback>>,
        )| {
            Box::pin(async move { Ok((Imports::fork(accessor, this, id, data, callback).await?,)) })
        },
    )?;
    host.func_wrap_concurrent(
        "[method]command-context.switch-session",
        |accessor,
         (this, path, options): (
            Resource<CommandContext>,
            String,
            Option<SwitchSessionOptions>,
        )| {
            Box::pin(
                async move { Ok((Imports::switch_session(accessor, this, path, options).await?,)) },
            )
        },
    )
}
