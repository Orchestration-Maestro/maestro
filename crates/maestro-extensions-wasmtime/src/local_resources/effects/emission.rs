//! Emit-time snapshots and split guest-local prefixes use the original native frame.
use super::{Capability, EmitterEndpoint, Frame, Message, Registry, Reply, State};
use maestro_extensions::Local;
use std::sync::mpsc;

/// Stable snapshot key followed by registration-ordered guest-local callback identities.
pub(crate) type Snapshot = (u32, Vec<Option<u32>>);
/// Retained native snapshot entries, including entries already consumed by the walk.
pub(super) type Snapshots =
    Vec<Option<Vec<Option<maestro_extensions::callbacks::CallbackDelivery<Local>>>>>;

/// Ordered snapshot operations, carried by the one existing effect receiver.
pub(crate) enum Request {
    /// Opens the independent native boundary of one guest-local listener.
    BeginLocal(u32, u32, mpsc::Sender<Result<EmitterEndpoint, String>>),
    /// Completes the guest-local listener through the original native policy.
    EndLocal(Option<String>, Reply),
    /// Retains the bus snapshot before guest-local traversal.
    Snapshot(String, String, mpsc::Sender<Result<Snapshot, String>>),
    /// Runs a consecutive host segment from the retained snapshot.
    Segment(u32, u32, u32, Reply),
    /// Releases one completed snapshot.
    EndSnapshot(u32, Reply),
}

impl EmitterEndpoint {
    /// Borrows a separate frame for one locally invoked listener.
    pub(crate) fn begin_local(&self, snapshot: u32, index: u32) -> Result<Self, String> {
        self.0.request(|id, reply| {
            Message::Effect(
                id,
                super::Request::Emission(Request::BeginLocal(snapshot, index, reply)),
            )
        })
    }
    /// Reports listener failure without aborting later snapshot entries.
    pub(crate) fn end_local(&self, error: Option<String>) -> crate::import_route::Route {
        self.0.route(|id, reply| {
            Message::Effect(
                id,
                super::Request::Emission(Request::EndLocal(error, reply)),
            )
        })
    }

    /// Returns emit-time tags without entering a guest instance.
    pub(crate) fn snapshot(&self, channel: String, data: String) -> Result<Snapshot, String> {
        self.0.request(|id, reply| {
            Message::Effect(
                id,
                super::Request::Emission(Request::Snapshot(channel, data, reply)),
            )
        })
    }
    /// Runs the native segment before the guest-local walk continues.
    pub(crate) fn segment(
        &self,
        snapshot: u32,
        start: u32,
        end: u32,
    ) -> crate::import_route::Route {
        self.0.route(|id, reply| {
            Message::Effect(
                id,
                super::Request::Emission(Request::Segment(snapshot, start, end, reply)),
            )
        })
    }
    /// Releases the snapshot on both successful and failed local traversal.
    pub(crate) fn end_snapshot(&self, snapshot: u32) -> Result<(), String> {
        self.0.request(|id, reply| {
            Message::Effect(
                id,
                super::Request::Emission(Request::EndSnapshot(snapshot, reply)),
            )
        })
    }
}
impl Registry {
    /// Records a weak listener identity for the guest-local snapshot tags.
    pub(crate) fn listener_identity(
        &self,
        listener: &maestro_extensions::EventListener<Local>,
        id: u32,
    ) {
        self.resources.borrow_mut().identity(
            identity(listener),
            crate::local_resources::Identity::Listener(std::rc::Rc::downgrade(listener), id),
        );
    }
}
impl State {
    /// Executes snapshot effects outside the registry and frame table borrows.
    pub(super) fn emission(&self, capability: Option<&Capability>, request: Request) {
        let frame = match capability {
            Some(Capability::CallbackFrame(frame)) => Ok(frame),
            _ => Err("extension callback-emitter resource ended".to_owned()),
        };
        match request {
            Request::BeginLocal(snapshot, index, reply) => {
                let _ = reply.send(
                    frame.and_then(|frame| self.registry.begin_local(frame, snapshot, index)),
                );
            }
            Request::EndLocal(error, reply) => {
                let _ = reply.send(frame.and_then(|frame| frame.finish_local(error)));
            }
            Request::Snapshot(channel, data, reply) => {
                let _ = reply
                    .send(frame.and_then(|frame| self.registry.snapshot(frame, &channel, &data)));
            }
            Request::Segment(snapshot, start, end, reply) => {
                let _ = reply.send(frame.and_then(|frame| frame.segment(snapshot, start, end)));
            }
            Request::EndSnapshot(snapshot, reply) => {
                let _ = reply.send(frame.and_then(|frame| frame.end_snapshot(snapshot)));
            }
        }
    }
}
impl Registry {
    /// Opens a guest-local prefix without calling back into its active instance.
    fn begin_local(
        &self,
        frame: &Frame,
        snapshot: u32,
        index: u32,
    ) -> Result<EmitterEndpoint, String> {
        let invocation = frame
            .take_entry(snapshot, index)?
            .begin()
            .map_err(|error| error.to_string())?;
        self.insert(Capability::CallbackFrame(Frame {
            native: invocation.frame(),
            snapshots: std::cell::RefCell::default(),
            local: std::cell::RefCell::new(Some(invocation)),
            tail: std::cell::RefCell::new(super::Tail::Open(None)),
        }))
        .map(EmitterEndpoint)
    }
    /// Tags the real bus snapshot by native callback identity, one lookup per entry.
    fn snapshot(&self, frame: &Frame, channel: &str, data: &str) -> Result<Snapshot, String> {
        let data = crate::writer_reads::decode(data)?;
        let entries = frame
            .native
            .snapshot(channel, data)
            .map_err(|error| error.to_string())?;
        let resources = self.resources.borrow();
        let tags = entries
            .iter()
            .map(|entry| {
                inspect();
                resources
                    .identities
                    .get(&identity(entry.listener()))
                    .and_then(|identity| match identity {
                        crate::local_resources::Identity::Listener(_, id) => Some(*id),
                        crate::local_resources::Identity::Schema(_)
                        | crate::local_resources::Identity::EditorFactory(..) => None,
                    })
            })
            .collect();
        let mut snapshots = frame.snapshots.borrow_mut();
        let id = u32::try_from(snapshots.len()).map_err(|error| error.to_string())?;
        snapshots.push(Some(entries.into_iter().map(Some).collect()));
        Ok((id, tags))
    }
}
impl Frame {
    /// Drops retained snapshot ownership after both successful and failed traversal.
    fn end_snapshot(&self, snapshot: u32) -> Result<(), String> {
        self.snapshots
            .borrow_mut()
            .get_mut(snapshot as usize)
            .and_then(Option::take)
            .map(drop)
            .ok_or_else(|| "extension event snapshot ended".to_owned())
    }
    /// Transfers one retained entry exactly once to its guest-local prefix.
    fn take_entry(
        &self,
        snapshot: u32,
        index: u32,
    ) -> Result<maestro_extensions::callbacks::CallbackDelivery<Local>, String> {
        self.snapshots
            .borrow_mut()
            .get_mut(snapshot as usize)
            .and_then(Option::as_mut)
            .and_then(|entries| entries.get_mut(index as usize))
            .and_then(Option::take)
            .ok_or_else(|| "extension event snapshot entry ended".to_owned())
    }
    /// Closes aliases before dispatching foreign prefixes and handing off the listener tail.
    fn finish_local(&self, error: Option<String>) -> Result<(), String> {
        let invocation = self
            .local
            .borrow_mut()
            .take()
            .ok_or_else(|| "extension local listener prefix ended".to_owned())?;
        let tail = self.take_tail();
        let result = match error {
            Some(error) => Err(error.into()),
            None => tail.map_err(Into::into),
        };
        invocation.finish(result);
        Ok(())
    }

    /// Takes the retained segment before invoking native listeners outside table borrows.
    fn segment(&self, snapshot: u32, start: u32, end: u32) -> Result<(), String> {
        let entries = {
            let mut snapshots = self.snapshots.borrow_mut();
            let entries = snapshots
                .get_mut(snapshot as usize)
                .and_then(Option::as_mut)
                .ok_or_else(|| "extension event snapshot ended".to_owned())?;
            let segment = entries
                .get_mut(start as usize..end as usize)
                .ok_or_else(|| "extension event snapshot range invalid".to_owned())?;
            if segment.iter().any(Option::is_none) {
                return Err("extension event snapshot entry ended".to_owned());
            }
            segment
                .iter_mut()
                .filter_map(Option::take)
                .collect::<Vec<_>>()
        };
        for entry in entries {
            entry.deliver().map_err(|error| error.to_string())?;
        }
        Ok(())
    }
}

/// The allocation address identifying one native listener.
fn identity(listener: &maestro_extensions::EventListener<Local>) -> usize {
    std::rc::Rc::as_ptr(listener).cast::<()>().addr()
}

#[cfg(test)]
thread_local! {
    /// Listener identities inspected by registration and snapshot tagging.
    static INSPECTIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    /// Map capacity, empty buckets included, that identity pruning scans.
    pub(crate) static CAPACITY: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Counts one inspected listener identity.
pub(crate) fn inspect() {
    #[cfg(test)]
    INSPECTIONS.set(INSPECTIONS.get() + 1);
}

#[cfg(test)]
mod tests;
