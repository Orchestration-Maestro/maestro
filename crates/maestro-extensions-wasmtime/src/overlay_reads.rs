//! Retained overlay options and handles on the existing local widget request owner.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{
    guest_calls::Handle,
    import_route::Value,
    local_resources::{Lease, Message, held::Options},
    widget_reads::{Reply, WidgetMessage},
};
use std::sync::Arc;

/// One operation on a shown overlay.
#[derive(Clone, Copy, wasmtime::component::ComponentType, wasmtime::component::Lift)]
#[component(variant)]
pub(crate) enum HandleRequest {
    /// Removes the overlay for good.
    #[component(name = "hide")]
    Hide,
    /// Hides or shows the overlay again.
    #[component(name = "set-hidden")]
    SetHidden(bool),
    /// Whether the overlay is temporarily hidden.
    #[component(name = "is-hidden")]
    IsHidden,
    /// Focuses the overlay and brings it to the front.
    #[component(name = "focus")]
    Focus,
    /// Returns focus to the previous target.
    #[component(name = "unfocus")]
    Unfocus,
    /// Whether the overlay has focus.
    #[component(name = "is-focused")]
    IsFocused,
}

/// Overlay requests share the widget pump and resource table.
pub(crate) enum OverlayMessage {
    /// Creates one shared options record.
    NewOptions(String, Reply<OptionsEndpoint>),
    /// Operates on the shown overlay with this identity.
    Handle(u64, HandleRequest, Reply<Value>),
}

/// Store-safe ownership of the original native options record.
#[derive(Clone)]
pub struct OptionsEndpoint(pub(crate) Arc<Lease>);
impl OptionsEndpoint {
    /// Reads current placement.
    pub(crate) fn get(&self) -> Result<String, String> {
        self.0.run::<Options, _>(|placement, _| {
            crate::overlay_layout::encode(&placement.options.borrow())
        })
    }

    /// Replaces placement without changing visibility policy.
    pub(crate) fn set(&self, encoded: String) -> Result<(), String> {
        self.0.run::<Options, _>(move |placement, _| {
            let mut replacement = crate::overlay_layout::decode(&encoded)?;
            replacement
                .visible
                .clone_from(&placement.options.borrow().visible);
            let previous = placement.options.replace(replacement);
            drop(previous);
            Ok(())
        })
    }

    /// Lends another owner of the same record.
    pub(crate) fn share(&self) -> Result<OptionsEndpoint, String> {
        self.0
            .run::<Options, _>(|placement, registry| registry.lend_placement(placement.clone()))
    }

    /// Replaces or removes the authored visibility check of this record.
    pub(crate) fn set_visible(
        &self,
        api: crate::api_reads::Endpoint,
        visible: Option<Handle>,
    ) -> Result<(), String> {
        self.0.run::<Options, _>(move |placement, registry| {
            registry.set_visible(placement, &api, visible)
        })
    }
}

/// Store-safe ownership of a shown overlay's native handle.
#[derive(Clone)]
pub struct HandleEndpoint(pub(crate) Arc<Lease>);
impl HandleEndpoint {
    /// Runs one request; focus changes may call back into this instance.
    pub(crate) fn request(&self, request: HandleRequest) -> crate::import_route::Route {
        self.0.route(|id, reply| {
            Message::Widgets(WidgetMessage::Overlay(OverlayMessage::Handle(
                id, request, reply,
            )))
        })
    }
}
