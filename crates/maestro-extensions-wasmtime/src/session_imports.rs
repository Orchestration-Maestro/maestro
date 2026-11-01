//! Session-changing canonical imports retain real setup and continuation callbacks.
use crate::{
    callback_imports::{self, CallbackView},
    command_imports::{SessionChangeResult, endpoint},
    command_reads::Request,
    context_imports::CommandContext,
    guest_calls::Callback,
    session_calls::{NewSession, NewSessionData, Switch},
};
use wasmtime::component::{ComponentType, Lift, LinkerInstance, Resource};

/// A present canonical switch record may contain no continuation.
#[derive(ComponentType, Lift)]
#[component(record)]
struct SwitchOptions {
    /// Optional guest continuation; its absence does not erase this record.
    #[component(name = "with-session")]
    with_session: Option<Resource<Callback>>,
}

/// Complete owned operands of the canonical new-session import.
type NewSessionArgs = (
    Resource<CommandContext>,
    Option<NewSessionData>,
    Option<Resource<Callback>>,
    Option<Resource<Callback>>,
);

/// Installs all three session-changing operations on the existing command owner.
pub(crate) fn add_to_linker<T: CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    host.func_wrap_concurrent(
        "[method]command-context.new-session",
        |accessor, (resource, data, setup, with_session): NewSessionArgs| {
            Box::pin(async move {
                let context = endpoint(accessor, &resource)?;
                let options = accessor.with(|mut access| {
                    let store = access.get();
                    Ok::<_, wasmtime::Error>(NewSession {
                        data,
                        setup: callback_imports::optional(store, setup.as_ref())?,
                        with_session: callback_imports::optional(store, with_session.as_ref())?,
                    })
                })?;
                Ok((context
                    .command(|reply| Request::NewSession(options, reply))
                    .await
                    .map(SessionChangeResult::from),))
            })
        },
    )?;
    host.func_wrap_concurrent(
        "[method]command-context.fork",
        |accessor,
         (resource, id, data, callback): (
            Resource<CommandContext>,
            String,
            Option<String>,
            Option<Resource<Callback>>,
        )| {
            Box::pin(async move {
                let context = endpoint(accessor, &resource)?;
                let callback = accessor.with(|mut access| {
                    callback_imports::optional(access.get(), callback.as_ref())
                })?;
                Ok((context
                    .command(|reply| Request::Fork(id, data, callback, reply))
                    .await
                    .map(SessionChangeResult::from),))
            })
        },
    )?;
    host.func_wrap_concurrent("[method]command-context.switch-session", |accessor, (resource, path, options): (Resource<CommandContext>, String, Option<SwitchOptions>)| {
        Box::pin(async move {
            let context = endpoint(accessor, &resource)?;
            let options = accessor.with(|mut access| options.map(|options| callback_imports::optional(access.get(), options.with_session.as_ref()).map(|with_session| Switch { with_session })).transpose())?;
            Ok((context.command(|reply| Request::Switch(path, options, reply)).await.map(SessionChangeResult::from),))
        })
    })?;
    Ok(())
}
