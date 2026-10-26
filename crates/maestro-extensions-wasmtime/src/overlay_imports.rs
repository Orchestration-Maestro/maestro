//! Canonical overlay resources delegate to the existing widget owner.
use crate::{
    callback_imports::CallbackView,
    local_resources::Message,
    overlay_reads::{OptionsEndpoint, OptionsRequest, OverlayMessage},
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
    )
}
