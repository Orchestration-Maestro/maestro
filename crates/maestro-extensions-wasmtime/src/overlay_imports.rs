//! Canonical overlay resources delegate to the existing widget owner.
use crate::{
    callback_imports::CallbackView,
    guest_calls::Callback,
    local_resources::Message,
    overlay_reads::{
        HandleEndpoint, HandleRequest, OptionsEndpoint, OptionsRequest, OverlayMessage, Visibility,
    },
    widget_reads::WidgetMessage,
};
use wasmtime::{
    StoreContextMut,
    component::{LinkerInstance, Resource},
};

/// Installs the retained options constructor and placement operations.
pub(crate) fn add_to_linker<T: CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    crate::context_imports::add_resource::<T, OptionsEndpoint>(host, "overlay-options")?;
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
    host.func_wrap(
        "[method]overlay-options.get",
        |mut store: StoreContextMut<'_, T>, (this,): (Resource<OptionsEndpoint>,)| {
            let options = store.data_mut().ctx().table.get(&this)?.clone();
            Ok((options.read(OptionsRequest::Get),))
        },
    )?;
    host.func_wrap(
        "[method]overlay-options.set",
        |mut store: StoreContextMut<'_, T>,
         (this, encoded): (Resource<OptionsEndpoint>, String)| {
            let options = store.data_mut().ctx().table.get(&this)?.clone();
            Ok((options.read(|reply| OptionsRequest::Set(encoded, reply)),))
        },
    )?;
    shared(host)
}

/// Installs sharing and visibility of an options record and the shown overlay's handle.
fn shared<T: CallbackView + 'static>(host: &mut LinkerInstance<'_, T>) -> wasmtime::Result<()> {
    host.func_wrap(
        "[method]overlay-options.share",
        |mut store: StoreContextMut<'_, T>, (this,): (Resource<OptionsEndpoint>,)| {
            let options = store.data_mut().ctx().table.get(&this)?.clone();
            let shared = options
                .read(OptionsRequest::Share)
                .map_err(wasmtime::Error::msg)?;
            Ok((store.data_mut().ctx().table.push(shared)?,))
        },
    )?;
    host.func_wrap(
        "[method]overlay-options.set-visible",
        |mut store: StoreContextMut<'_, T>,
         (this, visible): (Resource<OptionsEndpoint>, Option<Resource<Callback>>)| {
            let store = store.data_mut();
            let options = store.ctx().table.get(&this)?.clone();
            let visible = crate::callback_imports::optional(store, visible.as_ref())?;
            let result = store.api().0.request(|api, reply| {
                Message::Widgets(WidgetMessage::Overlay(OverlayMessage::Visible(
                    Visibility {
                        api,
                        options,
                        visible,
                        reply,
                    },
                )))
            });
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
