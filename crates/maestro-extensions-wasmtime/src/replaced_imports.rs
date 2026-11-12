//! Canonical continuation methods preserve the creating session's native operations.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{
    bindings::maestro::extension::host::HostReplacedSessionContext,
    context_imports::{CommandContext, add_resource},
    lifecycle::State,
    read_imports::{endpoint, forwards, release},
    replaced_reads::Endpoint,
};
use wasmtime::component::{LinkerInstance, Resource};
use wasmtime_wasi::WasiView;

impl HostReplacedSessionContext for State {
    fn command(&mut self, this: Resource<Endpoint>) -> wasmtime::Result<Resource<CommandContext>> {
        let command = endpoint(self, &this)?
            .command()
            .map_err(wasmtime::Error::msg)?;
        Ok(self.ctx().table.push(CommandContext(command))?)
    }

    fn drop(&mut self, this: Resource<Endpoint>) -> wasmtime::Result<()> {
        release(self, this)
    }
}

/// Registers the continuation context with its synchronous method forwarded to the trait.
pub(crate) fn add_replaced(host: &mut LinkerInstance<'_, State>) -> wasmtime::Result<()> {
    add_resource::<State, Endpoint>(host, "replaced-session-context")?;
    forwards! { host, "replaced-session-context", HostReplacedSessionContext, Endpoint,
        "command" => command();
    }
    Ok(())
}

/// Defines the independently retained continuation context and its real message routes.
pub(crate) fn add_to_linker<T: WasiView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    host.func_wrap_concurrent(
        "[method]replaced-session-context.send-message",
        |accessor, (resource, message, options): (Resource<Endpoint>, String, Option<String>)| {
            Box::pin(async move {
                let endpoint =
                    accessor.with(|mut access| access.get().ctx().table.get(&resource).cloned())?;
                Ok((endpoint.send_message(message, options).await,))
            })
        },
    )?;
    host.func_wrap_concurrent(
        "[method]replaced-session-context.send-user-message",
        |accessor, (resource, value, options): (Resource<Endpoint>, String, Option<String>)| {
            Box::pin(async move {
                let endpoint =
                    accessor.with(|mut access| access.get().ctx().table.get(&resource).cloned())?;
                Ok((endpoint.send_user_message(value, options).await,))
            })
        },
    )?;
    Ok(())
}
