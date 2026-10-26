//! Retained overlay options on the existing local widget request owner.
use crate::{
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
}
impl OptionsRequest {
    /// Settles an unavailable options owner.
    pub(crate) fn reject(self, error: &str) {
        match self {
            Self::Get(reply) => drop(reply.send(Err(error.to_owned()))),
            Self::Set(_, reply) => drop(reply.send(Err(error.to_owned()))),
        }
    }
}

/// Overlay requests share the widget pump and resource table.
pub(crate) enum OverlayMessage {
    /// Creates one shared options record.
    NewOptions(String, Reply<OptionsEndpoint>),
    /// Operates on the options record with this identity.
    Options(u64, OptionsRequest),
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
