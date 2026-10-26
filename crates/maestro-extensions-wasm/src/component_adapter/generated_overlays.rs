//! Custom component and overlay requests through the generated host bindings.

use super::route;
use crate::bindings::maestro::extension::host;
use crate::component_adapter::{custom, overlays::OverlayRequest};
use crate::types::{ExtensionFuture, ExtensionResult};

/// Closes a custom component, answering the host's first-close decision.
pub(super) fn close(close: &host::CustomClose) -> ExtensionResult<bool> {
    match route::value(&close.close())? {
        host::RouteValue::Boolean(accepted) => Ok(accepted),
        _ => Err("extension close result kind mismatch".into()),
    }
}

/// Shows a custom component with its wire options until the host closes it.
pub(super) fn custom<'a>(
    resource: &'a host::ExtensionUiContext,
    factory: &'a host::Callback,
    options: Option<custom::CustomOptionsRef<'a, super::Generated>>,
) -> ExtensionFuture<'a, ()> {
    let options = options.map(|options| host::CustomUiOptions {
        overlay: options.overlay,
        overlay_options: options.overlay_options.map(|source| match source {
            custom::OptionsSourceRef::Placement(options) => {
                host::OverlayOptionsSource::Placement(options)
            }
            custom::OptionsSourceRef::Resolver(resolve) => {
                host::OverlayOptionsSource::Resolver(resolve)
            }
        }),
        on_handle: options.on_handle,
    });
    Box::pin(async move { resource.custom(factory, options).await })
}

/// Runs one request on a shown overlay; a flag request answers its flag.
pub(super) fn request(
    handle: &host::OverlayHandle,
    request: OverlayRequest,
) -> ExtensionResult<Option<bool>> {
    let request = match request {
        OverlayRequest::Hide => host::OverlayRequest::Hide,
        OverlayRequest::SetHidden(hidden) => host::OverlayRequest::SetHidden(hidden),
        OverlayRequest::IsHidden => host::OverlayRequest::IsHidden,
        OverlayRequest::Focus => host::OverlayRequest::Focus,
        OverlayRequest::Unfocus => host::OverlayRequest::Unfocus,
        OverlayRequest::IsFocused => host::OverlayRequest::IsFocused,
    };
    match route::value(&handle.request(request))? {
        host::RouteValue::Unit => Ok(None),
        host::RouteValue::Boolean(flag) => Ok(Some(flag)),
        _ => Err("extension overlay result kind mismatch".into()),
    }
}
