//! Ordinary asynchronous calls retain their typed native capabilities.
use crate::{api_imports::ApiView, local_resources::SignalEndpoint};
use wasmtime::component::{LinkerInstance, Resource};

/// Installs the shared call transport with explicit execution as its only operation.
pub(crate) fn add_to_linker<T: ApiView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    host.func_wrap_concurrent(
        "call",
        |accessor,
         (operation, data, signal): (String, String, Option<Resource<SignalEndpoint>>)| {
            Box::pin(async move {
                let (api, signal) = accessor.with(|mut access| {
                    let state = access.get();
                    let signal = signal
                        .map(|signal| state.ctx().table.get(&signal).cloned())
                        .transpose()?;
                    Ok::<_, wasmtime::Error>((state.api().clone(), signal))
                })?;
                let result = if operation == "exec" {
                    api.exec(crate::exec::Request { data, signal }).await
                } else {
                    Err(format!("unknown extension operation {operation}"))
                };
                Ok((result,))
            })
        },
    )
}
