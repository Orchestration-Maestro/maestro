//! Retained native watch registrations on the existing local resource pump.
use super::{Capability, Lease, Registry, pump::State};
use crate::guest_calls::{Handle, SynchronousKind};
use std::{
    rc::Rc,
    sync::{Arc, mpsc::Sender},
};

/// Store-safe lease of one native registration.
#[derive(Clone)]
pub(crate) struct Watcher(pub(crate) Arc<Lease>);
/// Operations on the original watch owner.
pub(crate) enum Request {
    /// Registers the authored callback under its creating facade.
    Open(String, Handle, Sender<Result<Watcher, String>>),
    /// Stops delivery from that same registration.
    Close(Sender<Result<(), String>>),
}
impl State {
    /// Resolves owners outside capability-table borrows before native calls.
    pub(super) fn watch(&self, id: u64, request: Request) {
        match request {
            Request::Open(path, listener, reply) => {
                let result = self.registry.watch(id, path, listener);
                let _ = reply.send(result);
            }
            Request::Close(reply) => {
                let capability = self.registry.capability(id);
                let result = match capability.as_deref() {
                    Some(Capability::NativeWatcher(watch)) => watch.close(),
                    _ => Err("native watch resource ended".into()),
                };
                let _ = reply.send(result);
            }
        }
    }
}
impl Registry {
    /// Keeps the existing native watcher and callback identity without a second registry.
    fn watch(&self, api: u64, path: String, listener: Handle) -> Result<Watcher, String> {
        let capability = self.capability(api);
        let Some(Capability::Api(api)) = capability.as_deref() else {
            return Err("extension API resource ended".into());
        };
        let io = self
            .resources
            .borrow()
            .native_io
            .clone()
            .ok_or("native I/O capability unavailable")?;
        let (registry, events) = (self.clone(), api.events.clone());
        let watch = io.watch(
            path,
            Rc::new(move || {
                let _ = crate::compact_calls::notify(
                    &listener,
                    &registry,
                    &events,
                    SynchronousKind::Watch,
                    String::new(),
                );
            }),
        )?;
        self.insert(Capability::NativeWatcher(watch)).map(Watcher)
    }
}
