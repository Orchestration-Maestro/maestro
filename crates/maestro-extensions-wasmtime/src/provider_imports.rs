//! Awaited native hook resources preserve the caller's original invocation family.
use crate::{
    callback_imports::CallbackView,
    guest_calls::Callback,
    local_resources::{
        Message, SignalEndpoint,
        provider_controls::{Endpoint, Request},
    },
    model_calls::{Phase, Selection, WireSelection},
};
use wasmtime::component::{LinkerInstance, Resource};

/// Typed operands of the one canonical native-hook method.
type Operands = (
    Resource<Endpoint>,
    Phase,
    String,
    String,
    Option<Resource<SignalEndpoint>>,
    Option<Resource<Callback>>,
);

/// Link the retained resource into the same canonical host instance as model dispatch.
pub(crate) fn add_to_linker<T: CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    crate::context_imports::add_resource::<T, Endpoint>(host, "provider-controls")?;
    host.func_wrap_concurrent(
        "[method]provider-controls.invoke",
        |a, (controls, phase, data, model, signal, owner): Operands| {
            Box::pin(async move {
                let (controls, signal, owner) = a.with(|mut access| {
                    let state = access.get();
                    let controls = state.ctx().table.get(&controls)?.clone();
                    let signal = signal
                        .map(|signal| state.ctx().table.get(&signal).cloned())
                        .transpose()?;
                    let owner = owner
                        .map(|owner| crate::callback_imports::retain_object(state, &owner))
                        .transpose()?;
                    Ok::<_, wasmtime::Error>((controls, signal, owner))
                })?;
                let result: Result<Selection, String> = controls
                    .0
                    .request_async(|id, reply, admission| {
                        Message::Provider(crate::local_resources::provider_controls::Command::Hook(
                            id,
                            Request {
                                phase,
                                data,
                                model,
                                signal,
                                owner,
                                reply,
                            },
                            admission,
                        ))
                    })
                    .await;
                let result = match result {
                    Ok(result) => Ok(a.with(|mut access| {
                        Ok::<_, wasmtime::Error>(WireSelection {
                            data: result.data,
                            signal: result
                                .signal
                                .map(|signal| access.get().ctx().table.push(signal))
                                .transpose()?,
                            owner: None,
                        })
                    })?),
                    Err(error) => Err(error),
                };
                Ok((result,))
            })
        },
    )
}
