//! Event callbacks: their registrations and their live delivery.
use super::{Inner, RpcClient, RpcClientError, RpcEventListener};
use maestro_models::records::listeners::Listeners;
use serde_json::Value;
use std::rc::Rc;

/// What the child's driver hands every registration.
pub(super) enum ClientInput<'a> {
    /// A record the child wrote that answers no pending request.
    Event(&'a Value),
}

/// One registration; every subscription gets a fresh one, so a callback registered twice
/// is called twice.
pub(super) type ClientListener = Rc<dyn Fn(&ClientInput<'_>) -> Result<(), RpcClientError>>;

impl RpcClient {
    /// Registers `listener` for every later record the child writes that answers no
    /// pending request, in registration order. The returned operation removes this
    /// registration only; calling it again does nothing, and dropping it keeps the
    /// registration.
    pub fn on_event(&self, listener: RpcEventListener) -> impl Fn() + use<> {
        let registration: ClientListener = Rc::new(move |input: &ClientInput<'_>| match input {
            ClientInput::Event(event) => listener(event),
        });
        Listeners::subscribe(&self.inner.listeners, registration)
    }
}

impl Inner {
    /// Calls the registrations with `event` in order, including ones added meanwhile; the
    /// first failure ends this delivery.
    pub(super) fn dispatch(&self, event: &Value) {
        let _ = Listeners::try_for_each_live(&self.listeners, |listener| {
            listener(&ClientInput::Event(event))
        });
    }
}
