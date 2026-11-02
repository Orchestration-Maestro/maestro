//! Custom component and overlay requests through the generated host bindings.

use super::route;
use crate::bindings::maestro::extension::host;
use crate::component_adapter::{custom, overlays::OverlayRequest};
use crate::types::{ExtensionFuture, ExtensionResult};

/// Implements the custom component and overlay imports inside the generated imports.
macro_rules! overlay_forwarding {
    () => {
        type CustomClose = host::CustomClose;
        fn custom_close(&self, close: &host::CustomClose) -> ExtensionResult<bool> {
            overlays::close(close)
        }
        fn ui_custom<'a>(
            &'a self,
            resource: &'a host::ExtensionUiContext,
            factory: &'a host::Callback,
            options: Option<super::custom::CustomOptionsRef<'a, Self>>,
        ) -> ExtensionFuture<'a, ()> {
            overlays::custom(resource, factory, options)
        }
        type OverlayOptions = host::OverlayOptions;
        forward_resources! { functions {
            new_overlay_options(encoded: &str) -> ExtensionResult<host::OverlayOptions> => host::new_overlay_options;
        } }
        forward_resources! {
            overlay_options_get(options: &host::OverlayOptions) -> ExtensionResult<String> => get();
            overlay_options_set(options: &host::OverlayOptions, encoded: &str) -> ExtensionResult<()> => set(encoded);
            overlay_options_share(options: &host::OverlayOptions) -> host::OverlayOptions => share();
            overlay_options_set_visible(options: &host::OverlayOptions, visible: Option<&host::Callback>) -> ExtensionResult<()> => set_visible(visible);
        }
        type OverlayHandle = host::OverlayHandle;
        fn overlay_handle_request(
            &self,
            handle: &host::OverlayHandle,
            request: super::overlays::OverlayRequest,
        ) -> ExtensionResult<Option<bool>> {
            overlays::request(handle, request)
        }
    };
}

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
