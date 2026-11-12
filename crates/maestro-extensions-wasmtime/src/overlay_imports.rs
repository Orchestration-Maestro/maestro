//! Canonical overlay resources delegate to the existing widget owner.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{
    api_imports::ApiView,
    bindings::maestro::extension::host::HostOverlayOptions,
    callback_imports::{CallbackView, optional},
    context_imports::add_resource,
    guest_calls::Callback,
    lifecycle::State,
    local_resources::Message,
    overlay_reads::{HandleEndpoint, HandleRequest, OptionsEndpoint, OverlayMessage},
    read_imports::{endpoint, forwards, release},
    widget_reads::WidgetMessage,
};
use wasmtime::{
    StoreContextMut,
    component::{LinkerInstance, Resource},
};
use wasmtime_wasi::WasiView;

/// Installs the retained options constructor and the shown overlay's handle.
pub(crate) fn add_to_linker<T: CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    host.func_wrap(
        "new-overlay-options",
        |mut store: StoreContextMut<'_, T>, (encoded,): (String,)| {
            let result = store.data().api().0.request(|_, reply| {
                Message::Widgets(WidgetMessage::Overlay(OverlayMessage::NewOptions(
                    encoded, reply,
                )))
            });
            let result = match result {
                Ok(options) => Ok(store.data_mut().ctx().table.push(options)?),
                Err(error) => Err(error),
            };
            Ok((result,))
        },
    )?;
    crate::context_imports::add_resource::<T, HandleEndpoint>(host, "overlay-handle")?;
    host.func_wrap(
        "[method]overlay-handle.request",
        |mut store: StoreContextMut<'_, T>,
         (this, request): (Resource<HandleEndpoint>, HandleRequest)| {
            let handle = store.data_mut().ctx().table.get(&this)?.clone();
            Ok((crate::import_route::push(
                store.data_mut(),
                handle.request(request),
            )?,))
        },
    )
}

impl HostOverlayOptions for State {
    fn get(&mut self, this: Resource<OptionsEndpoint>) -> wasmtime::Result<Result<String, String>> {
        Ok(endpoint(self, &this)?.get())
    }

    fn set(
        &mut self,
        this: Resource<OptionsEndpoint>,
        placement: String,
    ) -> wasmtime::Result<Result<(), String>> {
        Ok(endpoint(self, &this)?.set(placement))
    }

    fn share(
        &mut self,
        this: Resource<OptionsEndpoint>,
    ) -> wasmtime::Result<Resource<OptionsEndpoint>> {
        let shared = endpoint(self, &this)?
            .share()
            .map_err(wasmtime::Error::msg)?;
        Ok(self.ctx().table.push(shared)?)
    }

    fn set_visible(
        &mut self,
        this: Resource<OptionsEndpoint>,
        visible: Option<Resource<Callback>>,
    ) -> wasmtime::Result<Result<(), String>> {
        let options = endpoint(self, &this)?;
        let visible = optional(self, visible.as_ref())?;
        let api = self.api().clone();
        Ok(options.set_visible(api, visible))
    }

    fn drop(&mut self, this: Resource<OptionsEndpoint>) -> wasmtime::Result<()> {
        release(self, this)
    }
}

/// Forwards every overlay options method to its trait implementation.
pub(crate) fn add_options(host: &mut LinkerInstance<'_, State>) -> wasmtime::Result<()> {
    add_resource::<State, OptionsEndpoint>(host, "overlay-options")?;
    forwards! { host, "overlay-options", HostOverlayOptions, OptionsEndpoint,
        "get" => get();
        "set" => set(placement: String);
        "share" => share();
        "set-visible" => set_visible(visible: Option<Resource<Callback>>);
    }
    Ok(())
}
