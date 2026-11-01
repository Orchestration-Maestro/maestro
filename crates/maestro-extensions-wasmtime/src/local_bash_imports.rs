//! Canonical local shell resources retain native operations and scoped raw observers.
use crate::{
    callback_imports::CallbackView,
    local_resources::local_bash::{Endpoint, Execution},
};
use wasmtime::{
    StoreContextMut,
    component::{LinkerInstance, Resource},
};

/// Installs lazy native construction and asynchronous execution on the existing local pump.
pub(crate) fn add_to_linker<T: CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    crate::context_imports::add_resource::<T, Endpoint>(host, "local-bash-operations")?;
    host.func_wrap(
        "create-local-bash-operations",
        |mut store: StoreContextMut<'_, T>, (shell_path,): (Option<String>,)| {
            let result = store.data().api().local_shell(shell_path);
            let result = match result {
                Ok(operations) => Ok(store.data_mut().ctx().table.push(operations)?),
                Err(error) => Err(error),
            };
            Ok((result,))
        },
    )?;
    host.func_wrap_concurrent(
        "[method]local-bash-operations.exec",
        |accessor, (operations, request, signal, output): ExecutionArgs| {
            Box::pin(async move {
                let (operations, execution) = accessor.with(|mut access| {
                    let state = access.get();
                    Ok::<_, wasmtime::Error>((
                        state.ctx().table.get(&operations)?.clone(),
                        Execution {
                            request,
                            signal: signal
                                .as_ref()
                                .map(|signal| state.ctx().table.get(signal).cloned())
                                .transpose()?,
                            output: crate::callback_imports::retain_object(state, &output)?,
                            api: state.api().clone(),
                        },
                    ))
                })?;
                Ok((operations.execute(execution).await,))
            })
        },
    )?;
    Ok(())
}

/// Raw shell data and cancellation remain distinct typed capabilities beside plain operands.
type ExecutionArgs = (
    Resource<Endpoint>,
    crate::guest_calls::bash::BashRequest,
    Option<Resource<crate::local_resources::SignalEndpoint>>,
    Resource<crate::guest_calls::Callback>,
);
