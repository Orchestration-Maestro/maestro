//! Custom imports retain original UI operands and use its existing admitted request pump.
use crate::{
    callback_imports::{CallbackView, retain},
    custom_reads::{CloseEndpoint, Options, Request, Source},
    guest_calls::Callback,
    overlay_reads::OptionsEndpoint,
    ui_reads::{Dialog, Endpoint},
};
use wasmtime::{
    StoreContextMut,
    component::{ComponentType, Lift, LinkerInstance, Resource},
};

/// Canonical placement: a borrowed record or an authored resolver.
#[derive(ComponentType, Lift)]
#[component(variant)]
enum WireSource {
    /// Original shared placement.
    #[component(name = "placement")]
    Placement(Resource<OptionsEndpoint>),
    /// Authored resolver run when the overlay is shown.
    #[component(name = "resolver")]
    Resolver(Resource<Callback>),
}
/// Canonical presentation operands borrow existing resources.
#[derive(ComponentType, Lift)]
#[component(record)]
struct WireOptions {
    /// Original overlay-mode presence.
    overlay: Option<bool>,
    /// Placement, preserving its absence.
    #[component(name = "overlay-options")]
    overlay_options: Option<WireSource>,
    /// Authored receiver of the shown overlay's handle.
    #[component(name = "on-handle")]
    on_handle: Option<Resource<Callback>>,
}
impl WireOptions {
    /// Retains the original shared options endpoint and callbacks for the operation.
    fn retained<T: CallbackView>(self, store: &mut T) -> wasmtime::Result<Options> {
        let placement = match self.overlay_options {
            Some(WireSource::Placement(options)) => {
                Some(Source::Placement(store.ctx().table.get(&options)?.clone()))
            }
            Some(WireSource::Resolver(resolver)) => {
                Some(Source::Resolver(retain(store, &resolver)?))
            }
            None => None,
        };
        Ok(Options {
            overlay: self.overlay,
            placement,
            on_handle: crate::callback_imports::optional(store, self.on_handle.as_ref())?,
        })
    }
}
/// Installs the close resource and the asynchronous custom operation.
pub(crate) fn add_to_linker<T: CallbackView + 'static>(
    host: &mut LinkerInstance<'_, T>,
) -> wasmtime::Result<()> {
    crate::context_imports::add_resource::<T, CloseEndpoint>(host, "custom-close")?;
    host.func_wrap(
        "[method]custom-close.close",
        |mut store: StoreContextMut<'_, T>, (this,): (Resource<CloseEndpoint>,)| {
            let close = store.data_mut().ctx().table.get(&this)?.clone();
            Ok((crate::import_route::push(store.data_mut(), close.close())?,))
        },
    )?;
    host.func_wrap_concurrent("[method]extension-ui-context.custom", |accessor, (ui, factory, options): (Resource<Endpoint>, Resource<Callback>, Option<WireOptions>)| {
        Box::pin(async move {
            let (ui, request) = accessor.with(|mut access| {
                let store = access.get();
                let factory = retain(store, &factory)?;
                let api = store.api().clone();
                let options = options.map(|options| options.retained(store)).transpose()?;
                Ok::<_, wasmtime::Error>((store.ctx().table.get(&ui)?.clone(), Request { factory, api, options }))
            })?;
            Ok((ui.dialog(|reply| Dialog::Custom(request, reply), None, None).await,))
        })
    })
}
