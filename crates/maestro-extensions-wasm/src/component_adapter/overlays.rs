//! Overlay facades retain the original host options and handle capabilities.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::{Imports, lists::WidgetHandler};
use crate::{
    ExtensionResult, OverlayHandle, OverlayHandlePort, OverlayOptions, OverlayOptionsHandle,
    OverlayVisible, tui::OverlayOptionsPort,
};
use std::rc::Rc;

/// The host and its original shared options record.
pub(super) struct Options<I: Imports> {
    /// Adapter serving this record.
    imports: I,
    /// Original host resource.
    pub(super) resource: I::OverlayOptions,
}
impl<I: Imports> OverlayOptionsPort for Options<I> {
    fn get(&self) -> ExtensionResult<OverlayOptions> {
        let encoded = self.imports.overlay_options_get(&self.resource)?;
        serde_json::from_str(&encoded).map_err(|error| error.to_string())
    }
    fn set(&self, options: OverlayOptions) -> ExtensionResult<()> {
        let encoded = serde_json::to_string(&options).map_err(|error| error.to_string())?;
        self.imports.overlay_options_set(&self.resource, &encoded)
    }
    fn set_visible(&self, visible: Option<OverlayVisible>) -> ExtensionResult<()> {
        let staged = visible.map(|visible| {
            super::lists::stage_handler(&self.imports, WidgetHandler::Visible(visible))
        });
        let result = self.imports.overlay_options_set_visible(
            &self.resource,
            staged.as_ref().map(|staged| &staged.handle),
        );
        if let Some(staged) = staged {
            staged.settle(result.is_ok());
        }
        result
    }
}

/// Creates and retains the host's shared placement record.
pub(super) fn options<I: Imports>(
    imports: &I,
    options: &OverlayOptions,
) -> ExtensionResult<OverlayOptionsHandle> {
    let encoded = serde_json::to_string(&options).map_err(|error| error.to_string())?;
    let resource = imports.new_overlay_options(&encoded)?;
    let owner = Rc::new(Options {
        imports: imports.clone(),
        resource,
    });
    Ok(OverlayOptionsHandle(crate::Supplied::lent(
        owner.clone(),
        owner,
    )))
}

/// Another owner of the host record behind `options`, for transfer to the host.
///
/// # Errors
/// Returns a message when the placement belongs to another host.
pub(crate) fn share<I: Imports>(
    imports: &I,
    options: &OverlayOptionsHandle,
) -> ExtensionResult<I::OverlayOptions> {
    let owner = options
        .0
        .host::<Options<I>>()
        .ok_or_else(|| "overlay options belong to another host".to_owned())?;
    Ok(imports.overlay_options_share(&owner.resource))
}

/// One operation on a shown overlay.
#[derive(Clone, Copy)]
pub(crate) enum OverlayRequest {
    /// Removes the overlay for good.
    Hide,
    /// Hides or shows the overlay again.
    SetHidden(bool),
    /// Whether the overlay is temporarily hidden.
    IsHidden,
    /// Focuses the overlay.
    Focus,
    /// Returns focus to the previous target.
    Unfocus,
    /// Whether the overlay has focus.
    IsFocused,
}

/// The host and its shown overlay.
struct Handle<I: Imports> {
    /// Adapter serving the overlay.
    imports: I,
    /// Original host resource.
    resource: I::OverlayHandle,
}
impl<I: Imports> Handle<I> {
    /// Runs a request that answers nothing.
    fn act(&self, request: OverlayRequest) -> ExtensionResult<()> {
        self.imports
            .overlay_handle_request(&self.resource, request)
            .map(drop)
    }
    /// Runs a request that answers a flag.
    fn flag(&self, request: OverlayRequest) -> ExtensionResult<bool> {
        self.imports
            .overlay_handle_request(&self.resource, request)?
            .ok_or_else(|| "extension overlay result kind mismatch".to_owned())
    }
}
impl<I: Imports> OverlayHandlePort for Handle<I> {
    fn hide(&self) -> ExtensionResult<()> {
        self.act(OverlayRequest::Hide)
    }
    fn set_hidden(&self, hidden: bool) -> ExtensionResult<()> {
        self.act(OverlayRequest::SetHidden(hidden))
    }
    fn is_hidden(&self) -> ExtensionResult<bool> {
        self.flag(OverlayRequest::IsHidden)
    }
    fn focus(&self) -> ExtensionResult<()> {
        self.act(OverlayRequest::Focus)
    }
    fn unfocus(&self) -> ExtensionResult<()> {
        self.act(OverlayRequest::Unfocus)
    }
    fn is_focused(&self) -> ExtensionResult<bool> {
        self.flag(OverlayRequest::IsFocused)
    }
}

/// Retains a host overlay handle behind its facade.
pub(super) fn handle<I: Imports>(imports: &I, resource: I::OverlayHandle) -> OverlayHandle {
    OverlayHandle::new(Rc::new(Handle {
        imports: imports.clone(),
        resource,
    }))
}
