//! Retained overlay options and handles on the existing local widget request owner.
use crate::{
    guest_calls::Handle,
    import_route::Value,
    local_resources::{Lease, Message},
    widget_reads::{Reply, WidgetMessage},
};
use std::sync::Arc;

/// Placement operations on one retained options record.
pub(crate) enum OptionsRequest {
    /// Reads current placement.
    Get(Reply<String>),
    /// Replaces placement without changing visibility policy.
    Set(String, Reply<()>),
    /// Lends another owner of the same record.
    Share(Reply<OptionsEndpoint>),
}
impl OptionsRequest {
    /// Settles an unavailable options owner.
    pub(crate) fn reject(self, error: &str) {
        let error = error.to_owned();
        match self {
            Self::Get(reply) => drop(reply.send(Err(error))),
            Self::Set(_, reply) => drop(reply.send(Err(error))),
            Self::Share(reply) => drop(reply.send(Err(error))),
        }
    }
}

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

/// Replaces or removes the authored visibility check of one options record.
pub(crate) struct Visibility {
    /// The extension API whose bus the check runs on.
    pub(crate) api: u64,
    /// The options record.
    pub(crate) options: OptionsEndpoint,
    /// The authored check, absent to remove it.
    pub(crate) visible: Option<Handle>,
    /// Acknowledges the replacement.
    pub(crate) reply: Reply<()>,
}

/// Overlay requests share the widget pump and resource table.
pub(crate) enum OverlayMessage {
    /// Calls the original custom-component close operation.
    Close(u64, Reply<bool>),
    /// Creates one shared options record.
    NewOptions(String, Reply<OptionsEndpoint>),
    /// Operates on the options record with this identity.
    Options(u64, OptionsRequest),
    /// Replaces an options record's visibility check.
    Visible(Visibility),
    /// Operates on the shown overlay with this identity.
    Handle(u64, HandleRequest, Reply<Value>),
}

/// Store-safe ownership of the original native options record.
#[derive(Clone)]
pub(crate) struct OptionsEndpoint(pub(crate) Arc<Lease>);
impl OptionsEndpoint {
    /// Reads or changes placement without invoking extension callbacks.
    pub(crate) fn read<T>(
        &self,
        request: impl FnOnce(Reply<T>) -> OptionsRequest,
    ) -> Result<T, String> {
        self.0.request(|id, reply| {
            Message::Widgets(WidgetMessage::Overlay(OverlayMessage::Options(
                id,
                request(reply),
            )))
        })
    }
}

/// Store-safe ownership of a shown overlay's native handle.
#[derive(Clone)]
pub(crate) struct HandleEndpoint(pub(crate) Arc<Lease>);
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
