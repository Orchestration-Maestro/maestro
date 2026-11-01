//! Concurrent command imports await the operation admitted by the single local pump.
use crate::{command_reads::Request, context_imports::CommandContext};
use wasmtime::component::{ComponentType, LinkerInstance, Lower, Resource};
use wasmtime_wasi::WasiView;

/// Installs command operations using the original local pump and retained callbacks.
pub(crate) fn add_to_linker<T: crate::callback_imports::CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    for (method, request) in [
        ("wait-for-idle", Request::Wait as fn(_) -> _),
        ("reload", Request::Reload),
    ] {
        host.func_wrap_concurrent(
            &format!("[method]command-context.{method}"),
            move |accessor, (resource,): (Resource<CommandContext>,)| {
                Box::pin(async move {
                    let endpoint = endpoint(accessor, &resource)?;
                    Ok((endpoint.command(request).await,))
                })
            },
        )?;
    }
    host.func_wrap_concurrent(
        "[method]command-context.navigate-tree",
        |accessor, (resource, id, options): (Resource<CommandContext>, String, Option<String>)| {
            Box::pin(async move {
                let endpoint = endpoint(accessor, &resource)?;
                let result = match options
                    .as_deref()
                    .map(crate::writer_reads::decode)
                    .transpose()
                {
                    Ok(options) => endpoint
                        .command(|reply| Request::Navigate(id, options, reply))
                        .await
                        .map(SessionChangeResult::from),
                    Err(error) => Err(error),
                };
                Ok((result,))
            })
        },
    )?;
    Ok(())
}

/// Releases Store access before awaiting the original local command operation.
pub(crate) fn endpoint<T: WasiView + 'static>(
    accessor: &wasmtime::component::Accessor<T>,
    resource: &Resource<CommandContext>,
) -> wasmtime::Result<crate::context_reads::Endpoint> {
    accessor.with(|mut access| Ok(access.get().ctx().table.get(resource)?.0.clone()))
}

/// Canonical result preserves cancellation as a supplied typed boolean.
#[derive(ComponentType, Lower)]
#[component(record)]
pub(crate) struct SessionChangeResult {
    /// Whether the application cancelled this operation.
    cancelled: bool,
}

impl From<maestro_extensions::types::SessionChangeResult> for SessionChangeResult {
    fn from(result: maestro_extensions::types::SessionChangeResult) -> Self {
        Self {
            cancelled: result.cancelled,
        }
    }
}
