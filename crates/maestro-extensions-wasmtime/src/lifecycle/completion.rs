//! Once-only completion precedes native settlement and wakes the original Store receiver.
use crate::guest_calls::Call;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio::sync::mpsc::UnboundedSender;

/// One active Store invocation, shared only with its lifecycle reply token.
#[derive(Clone)]
pub(crate) struct Completion(Arc<State>);
/// Completion retains admission until its final notification has been queued.
struct State {
    /// Prevents fault cleanup and normal settlement from completing twice.
    completed: AtomicBool,
    /// Original Store receiver, not a second release dispatcher.
    calls: UnboundedSender<Call>,
}
impl Completion {
    /// Admits completion without retaining a sender in the Store itself.
    pub(crate) fn new(calls: UnboundedSender<Call>) -> Self {
        Self(Arc::new(State {
            completed: AtomicBool::new(false),
            calls,
        }))
    }
    /// Queues bookkeeping before exposing a reply to native reentry.
    pub(crate) fn finish(&self) {
        if !self.0.completed.swap(true, Ordering::AcqRel) {
            let _ = self.0.calls.send(Call::Finished);
        }
    }
}
impl Drop for Completion {
    fn drop(&mut self) {
        self.finish();
    }
}
