//! Native overlay resources retain the toolkit's existing shared options owner.
use super::{Capability, pump::State};
use crate::overlay_reads::{OptionsEndpoint, OptionsRequest, OverlayMessage};
use std::{cell::RefCell, rc::Rc};

impl State {
    /// Serves placement without borrowing the capability table across native effects.
    pub(super) fn overlays(&self, message: OverlayMessage) {
        match message {
            OverlayMessage::NewOptions(encoded, reply) => {
                let result = crate::overlay_layout::decode(&encoded)
                    .and_then(|options| {
                        self.registry
                            .insert(Capability::OverlayOptions(Rc::new(RefCell::new(options))))
                    })
                    .map(OptionsEndpoint);
                let _ = reply.send(result);
            }
            OverlayMessage::Options(id, request) => {
                let capability = self.registry.capability(id);
                match capability.as_deref() {
                    Some(Capability::OverlayOptions(options)) => answer(options, request),
                    _ => request.reject("extension overlay options resource ended"),
                }
            }
        }
    }
}

/// Reads or replaces placement on the same native record an overlay retains.
fn answer(options: &RefCell<maestro_extensions::widgets::OverlayOptions>, request: OptionsRequest) {
    match request {
        OptionsRequest::Get(reply) => {
            let _ = reply.send(crate::overlay_layout::encode(&options.borrow()));
        }
        OptionsRequest::Set(encoded, reply) => {
            let result = crate::overlay_layout::decode(&encoded).map(|mut replacement| {
                replacement.visible.clone_from(&options.borrow().visible);
                let previous = options.replace(replacement);
                drop(previous);
            });
            let _ = reply.send(result);
        }
    }
}
