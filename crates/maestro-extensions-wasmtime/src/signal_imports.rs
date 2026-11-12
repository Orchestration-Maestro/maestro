//! Canonical cancellation methods over the original signal the local owner retains.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{
    api_imports::ApiView,
    bindings::maestro::extension::host::HostAbortSignal,
    callback_imports::CallbackView,
    context_imports::add_resource,
    guest_calls::Callback,
    import_route::{Route, push},
    lifecycle::State,
    local_resources::SignalEndpoint,
    read_imports::{endpoint, forwards, release},
};
use wasmtime::{
    StoreContextMut,
    component::{LinkerInstance, Resource},
};
use wasmtime_wasi::WasiView;

impl HostAbortSignal for State {
    fn retain(
        &mut self,
        this: Resource<SignalEndpoint>,
    ) -> wasmtime::Result<Resource<SignalEndpoint>> {
        let signal = endpoint(self, &this)?;
        Ok(self.ctx().table.push(signal)?)
    }

    fn aborted(&mut self, this: Resource<SignalEndpoint>) -> wasmtime::Result<bool> {
        endpoint(self, &this)?
            .aborted()
            .map_err(wasmtime::Error::msg)
    }

    fn abort(&mut self, this: Resource<SignalEndpoint>) -> wasmtime::Result<Resource<Route>> {
        let route = endpoint(self, &this)?.abort();
        push(self, route)
    }

    fn drop(&mut self, this: Resource<SignalEndpoint>) -> wasmtime::Result<()> {
        release(self, this)
    }
}

/// Forwards every signal method to its trait implementation and installs the free functions
/// that create and observe signals.
pub(crate) fn add_to_linker(host: &mut LinkerInstance<'_, State>) -> wasmtime::Result<()> {
    add_resource::<State, SignalEndpoint>(host, "abort-signal")?;
    forwards! { host, "abort-signal", HostAbortSignal, SignalEndpoint,
        "retain" => retain();
        "aborted" => aborted();
        "abort" => abort();
    }
    host.func_wrap(
        "new-signal",
        |mut store: StoreContextMut<'_, State>, (): ()| {
            let result = match store.data().api().new_signal() {
                Ok(signal) => Ok(store.data_mut().ctx().table.push(signal)?),
                Err(error) => Err(error),
            };
            Ok((result,))
        },
    )?;
    host.func_wrap(
        "signal-subscribe",
        |mut store: StoreContextMut<'_, State>,
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
