//! Canonical cancellation imports retain the original native signal.
use crate::{
    callback_imports::CallbackView, guest_calls::Callback, local_resources::SignalEndpoint,
};
use wasmtime::{
    StoreContextMut,
    component::{LinkerInstance, Resource},
};

/// Installs abort observation on the original signal through the sole local request pump.
pub(crate) fn add_to_linker<T: CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    host.func_wrap(
        "signal-subscribe",
        |mut store: StoreContextMut<'_, T>,
         (signal, listener): (Resource<SignalEndpoint>, Resource<Callback>)| {
            let signal = store.data_mut().ctx().table.get(&signal)?.clone();
            let id = store.data_mut().ctx().table.get(&listener)?.id;
            let listener = store.data_mut().callbacks().observer(id);
            let api = store.data().api().clone();
            let result = match signal.subscribe(listener, api) {
                Ok(subscription) => Ok(store.data_mut().ctx().table.push(subscription)?),
                Err(error) => Err(error),
            };
            Ok((result,))
        },
    )
}
