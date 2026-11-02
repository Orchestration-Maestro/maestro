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
    host.func_wrap("new-signal", |mut store: StoreContextMut<'_, T>, (): ()| {
        let result = match store.data().api().new_signal() {
            Ok(signal) => Ok(store.data_mut().ctx().table.push(signal)?),
            Err(error) => Err(error),
        };
        Ok((result,))
    })?;
    host.func_wrap(
        "[method]abort-signal.abort",
        |mut store: StoreContextMut<'_, T>, (signal,): (Resource<SignalEndpoint>,)| {
            let signal = store.data_mut().ctx().table.get(&signal)?.clone();
            Ok((crate::import_route::push(store.data_mut(), signal.abort())?,))
        },
    )?;
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
