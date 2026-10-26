//! Once-only completion releases invocation admissions before native settlement.
use crate::guest_calls::{Call, Callback, Handle};
use std::sync::{Arc, Mutex, MutexGuard};
use tokio::sync::mpsc::UnboundedSender;

/// One active Store invocation, shared only with its lifecycle reply token.
#[derive(Clone)]
pub(crate) struct Completion(Arc<Mutex<Option<Operands>>>);
/// Strong operands are releasable even if Wasmtime retains the invocation future.
struct Operands {
    /// Original Store receiver, not a second release dispatcher.
    calls: Option<UnboundedSender<Call>>,
    /// Native owners remain admitted until completion, including cancelled callers.
    retained: Vec<Box<dyn Send>>,
}
impl Completion {
    /// Retains admission only until the once-only completion operation.
    pub(crate) fn new(calls: UnboundedSender<Call>) -> Self {
        Self(Arc::new(Mutex::new(Some(Operands {
            calls: Some(calls),
            retained: Vec::new(),
        }))))
    }
    /// Releases operands without announcing activity the Store driver never admitted.
    pub(crate) fn local() -> Self {
        Self(Arc::new(Mutex::new(Some(Operands {
            calls: None,
            retained: Vec::new(),
        }))))
    }
    /// Recovers the operand owner if a previous user abandoned its guard.
    fn operands(&self) -> MutexGuard<'_, Option<Operands>> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
    /// Moves callback strength out of the task's actual invocation operands.
    pub(super) fn callback(&self, handle: Handle) -> Callback {
        let callback = Callback {
            id: handle.id(),
            schema: None,
        };
        self.retain(handle);
        callback
    }
    /// Retains a live call operand in the same once-only release set.
    pub(super) fn retain(&self, operand: impl Send + 'static) {
        if let Some(operands) = self.operands().as_mut() {
            operands.retained.push(Box::new(operand));
        }
    }
    /// Releases all call operands before bookkeeping and native settlement.
    pub(crate) fn finish(&self) {
        let operands = self.operands().take();
        if let Some(Operands { calls, retained }) = operands {
            drop(retained);
            if let Some(calls) = calls {
                let _ = calls.send(Call::Finished);
            }
        }
    }
}
impl Drop for Completion {
    fn drop(&mut self) {
        self.finish();
    }
}

#[cfg(test)]
mod tests;
