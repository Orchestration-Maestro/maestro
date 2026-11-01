//! Retained custom presentation operands use the original native UI and widget owner.
use crate::{
    guest_calls::{
        Handle,
        widget_calls::{Answered, Event},
    },
    local_resources::{Lease, Message, Registry},
    overlay_reads::OptionsEndpoint,
};
use maestro_extensions::{
    ExtensionAPI, Local,
    types::{CustomUIOptions, OverlayOptionsSource, UIErrorReporter},
};
use std::sync::Arc;

/// Placement: the original shared record, or an authored resolver.
pub(crate) enum Source {
    /// The original shared native options record.
    Placement(OptionsEndpoint),
    /// Authored resolver, run when the overlay is shown.
    Resolver(Handle),
}
/// Presentation options with the authored callbacks they name.
pub(crate) struct Options {
    /// Whether the custom component is an overlay.
    pub(crate) overlay: Option<bool>,
    /// Placement, preserving its absence.
    pub(crate) placement: Option<Source>,
    /// Authored receiver of the shown overlay's handle.
    pub(crate) on_handle: Option<Handle>,
}
/// One custom factory and its original extension facade.
pub(crate) struct Request {
    /// Authored factory.
    pub(crate) factory: Handle,
    /// Original extension facade, not a replacement callback owner.
    pub(crate) api: crate::api_reads::Endpoint,
    /// Presentation options, preserving their absence.
    pub(crate) options: Option<Options>,
}
impl Request {
    /// Resolves native operands before delegating construction and settlement to the UI.
    pub(crate) fn start(
        self,
        ui: &maestro_extensions::ExtensionUIContext,
        registry: &Registry,
    ) -> Result<maestro_extensions::types::ExtensionUIFuture<()>, String> {
        let api = registry.facade(&self.api)?;
        let reporter = api.ui_error_reporter(ui).ok();
        let factory = self
            .factory
            .custom_factory(registry.clone(), &api, reporter.clone());
        let options = self
            .options
            .map(|options| options.native(registry, &api, reporter))
            .transpose()?;
        Ok(ui.custom_component(factory, options))
    }
}
impl Options {
    /// The native options: placement bound to the error reporter and handle callbacks that
    /// enter the guest within the host's callback frame.
    fn native(
        self,
        registry: &Registry,
        api: &ExtensionAPI<Local>,
        reporter: Option<UIErrorReporter>,
    ) -> Result<CustomUIOptions, String> {
        let overlay_options = match self.placement {
            Some(Source::Placement(options)) => Some(OverlayOptionsSource::Static(
                registry.overlay_options(&options, reporter.as_ref())?,
            )),
            Some(Source::Resolver(resolver)) => Some(OverlayOptionsSource::Resolver(resolve(
                resolver,
                registry.clone(),
                api,
                reporter,
            ))),
            None => None,
        };
        let on_handle = self.on_handle.map(|receiver| {
            let registry = registry.clone();
            api.ui_callback(move |handle, emitter| {
                let frame = registry.emitter(emitter.frame())?;
                let handle = registry.overlay_handle(handle)?;
                receiver.widget_in(&registry, frame, Event::OverlayHandle(handle))?;
                Ok(())
            })
        });
        Ok(CustomUIOptions {
            overlay: self.overlay,
            overlay_options,
            on_handle,
        })
    }
}
/// A native placement resolver that enters the authored one within the host's callback frame
/// and binds the record it answers to `reporter`.
fn resolve(
    resolver: Handle,
    registry: Registry,
    api: &ExtensionAPI<Local>,
    reporter: Option<UIErrorReporter>,
) -> maestro_extensions::types::OverlayOptionsResolver {
    api.ui_callback(move |(), emitter| {
        let frame = registry.emitter(emitter.frame())?;
        match resolver.widget_in(&registry, frame, Event::OverlayOptions)? {
            Answered::OverlayOptions(options) => {
                Ok(registry.overlay_options(&options, reporter.as_ref())?)
            }
            _ => Err("overlay options answer kind mismatch".into()),
        }
    })
}

/// Store-safe ownership of the native accepted-close operation.
#[derive(Clone)]
pub(crate) struct CloseEndpoint(pub(crate) Arc<Lease>);
impl CloseEndpoint {
    /// Calls the native close operation through the existing reentrant route.
    pub(crate) fn close(&self) -> crate::import_route::Route {
        self.0.route(|id, reply| {
            Message::Widgets(crate::widget_reads::WidgetMessage::Overlay(
                crate::overlay_reads::OverlayMessage::Close(id, reply),
            ))
        })
    }
}
