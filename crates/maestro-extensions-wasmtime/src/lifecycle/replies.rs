//! One keyed settlement owner survives guest traps without moving native senders into exports.
use std::{
    any::Any,
    collections::BTreeMap,
    marker::PhantomData,
    sync::{Arc, Mutex, MutexGuard},
};

/// Original asynchronous native result receiver.
type Response<T> = tokio::sync::oneshot::Receiver<Result<T, String>>;

/// Erased settlement retains its concrete result decoder and native sender.
trait Pending: Send {
    /// Delivers a result whose type is fixed by the private reply token.
    fn deliver(self: Box<Self>, result: Box<dyn Any + Send>) -> Result<(), String>;
    /// Rejects the original native waiter with the first Store fault.
    fn reject(self: Box<Self>, error: String);
}
/// The original native completion, never a second reply queue.
struct Settlement<T> {
    /// Runs outside the owner's mutex, including synchronous pump completion.
    settle: Box<dyn FnOnce(Result<T, String>) + Send>,
}
impl<T: Send + 'static> Pending for Settlement<T> {
    fn deliver(self: Box<Self>, result: Box<dyn Any + Send>) -> Result<(), String> {
        if let Ok(result) = result.downcast::<Result<T, String>>() {
            (self.settle)(*result);
            Ok(())
        } else {
            let error = "extension reply type mismatch".to_owned();
            (self.settle)(Err(error.clone()));
            Err(error)
        }
    }

    fn reject(self: Box<Self>, error: String) {
        (self.settle)(Err(error));
    }
}
/// One admitted reply and the driver's completion-before-settlement operation.
struct Entry {
    /// Typed native settlement behind its private reply token.
    pending: Box<dyn Pending>,
    /// Marks the Store invocation complete before native code can reenter.
    before: Option<Box<dyn FnOnce() + Send>>,
}
impl Entry {
    /// Removes lifecycle accounting before exposing any native result.
    fn completed(mut self) -> Box<dyn Pending> {
        if let Some(before) = self.before.take() {
            before();
        }
        self.pending
    }
}
/// Per-Store fault and keyed pending-call state.
#[derive(Default)]
struct State {
    /// Monotonic identities cannot collide with still-admitted calls.
    next: u64,
    /// Cached first failure; later calls never enter the guest.
    fault: Option<String>,
    /// Each original sender is owned here until exactly one settlement.
    pending: BTreeMap<u64, Entry>,
}
impl Drop for State {
    fn drop(&mut self) {
        let error = self
            .fault
            .as_deref()
            .unwrap_or("extension Store owner ended");
        for (_, entry) in std::mem::take(&mut self.pending) {
            entry.completed().reject(error.to_owned());
        }
    }
}
/// Shared access to the single Store lifecycle's reply and fault state.
#[derive(Clone, Default)]
pub(crate) struct Replies(Arc<Mutex<State>>);
impl Replies {
    /// Recovers state even if a panicking Store owner abandoned its lock.
    fn state(&self) -> MutexGuard<'_, State> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
    /// Retains the original asynchronous sender in the lifecycle, never in an export.
    pub(crate) fn reply<T: Send + 'static>(&self) -> Result<(Reply<T>, Response<T>), String> {
        let (sender, result) = tokio::sync::oneshot::channel();
        let reply = self.register(move |result| {
            let _ = sender.send(result);
        })?;
        Ok((reply, result))
    }

    /// Retains the native settlement before an invocation can reach the Store.
    pub(crate) fn register<T: Send + 'static>(
        &self,
        settle: impl FnOnce(Result<T, String>) + Send + 'static,
    ) -> Result<Reply<T>, String> {
        let mut state = self.state();
        if let Some(error) = state.fault.clone() {
            drop(state);
            settle(Err(error.clone()));
            return Err(error);
        }
        let id = state.next;
        state.next = id
            .checked_add(1)
            .ok_or("extension reply identity exhausted")?;
        state.pending.insert(
            id,
            Entry {
                pending: Box::new(Settlement {
                    settle: Box::new(settle),
                }),
                before: None,
            },
        );
        Ok(Reply {
            owner: self.clone(),
            id,
            result: PhantomData,
        })
    }
    /// Services the original pump until a typed synchronous guest reply settles.
    pub(crate) fn synchronous<T: Send + 'static>(
        &self,
        registry: &crate::local_resources::Registry,
        start: impl FnOnce(Reply<T>) -> Result<(), String>,
    ) -> Result<T, String> {
        self.synchronous_with(registry, None, |reply, _| start(reply))
    }
    /// Wait for a typed guest result while lending an invocation-local JSON operand.
    pub(crate) fn synchronous_with<T: Send + 'static>(
        &self,
        registry: &crate::local_resources::Registry,
        value: Option<&mut serde_json::Value>,
        start: impl FnOnce(Reply<T>, crate::schema_reads::Endpoint) -> Result<(), String>,
    ) -> Result<T, String> {
        self.synchronous_operand(
            registry,
            value.map_or(
                crate::local_resources::synchronous::Operand::None,
                crate::local_resources::synchronous::Operand::Json,
            ),
            start,
        )
    }
    /// Borrows one typed native operand until the original guest invocation settles.
    pub(crate) fn synchronous_operand<T: Send + 'static>(
        &self,
        registry: &crate::local_resources::Registry,
        operand: crate::local_resources::synchronous::Operand<'_>,
        start: impl FnOnce(Reply<T>, crate::schema_reads::Endpoint) -> Result<(), String>,
    ) -> Result<T, String> {
        let (sender, result) = std::sync::mpsc::channel();
        registry.invoke_scoped(operand, |completion, node| {
            let reply = self.register(move |value| {
                let _ = sender.send(value);
                completion.complete(Ok(()));
            })?;
            start(reply, node)
        })?;
        result.recv().map_err(|_| "extension guest reply ended")?
    }

    /// Caches the first fault and takes the pending map once, never rescanning per reply.
    pub(crate) fn fault(&self, error: String) -> String {
        let mut state = self.state();
        if let Some(error) = &state.fault {
            return error.clone();
        }
        state.fault = Some(error.clone());
        let pending = std::mem::take(&mut state.pending);
        drop(state);
        for (_, entry) in pending {
            entry.completed().reject(error.clone());
        }
        error
    }
}
/// Type-safe token; exports hold no native sender or fault-settlement ownership.
pub(crate) struct Reply<T> {
    /// Original per-Store reply owner.
    owner: Replies,
    /// Unique key, independent of authored callbacks or canonical resources.
    id: u64,
    /// Prevents using a token for any other result type.
    result: PhantomData<fn(T)>,
}
impl<T: Send + 'static> Reply<T> {
    /// Attaches the driver's once-only task completion before guest entry.
    pub(crate) fn before(&self, before: impl FnOnce() + Send + 'static) {
        if let Some(entry) = self.owner.state().pending.get_mut(&self.id) {
            entry.before = Some(Box::new(before));
        }
    }
    /// Settles once outside all guards; a prior Store fault already owns the reply.
    pub(crate) fn send(self, result: Result<T, String>) -> Result<(), String> {
        let pending = self.owner.state().pending.remove(&self.id);
        pending
            .ok_or_else(|| "extension reply already settled".to_owned())?
            .completed()
            .deliver(Box::new(result))
    }
}

impl<T> Drop for Reply<T> {
    fn drop(&mut self) {
        let mut state = self.owner.state();
        let unstarted = state
            .pending
            .get(&self.id)
            .is_some_and(|entry| entry.before.is_none());
        let pending = unstarted.then(|| state.pending.remove(&self.id)).flatten();
        drop(state);
        if let Some(entry) = pending {
            entry
                .completed()
                .reject("extension guest reply ended".into());
        }
    }
}

#[cfg(test)]
mod tests;
