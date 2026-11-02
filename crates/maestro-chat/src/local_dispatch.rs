//! Work a sendable producer hands to the local owners it updates.

use maestro_app::DiagnosticErrorInfo;
use maestro_tui::tui::TuiRuntime;
use std::{
    any::Any,
    cell::RefCell,
    collections::HashMap,
    marker::PhantomData,
    rc::Rc,
    sync::{
        Arc, Mutex, MutexGuard, PoisonError,
        atomic::{AtomicU64, Ordering},
        mpsc as sync,
    },
};
use tokio::sync::{mpsc, oneshot};

/// What applying one item reports.
type Outcome = Result<(), DiagnosticErrorInfo>;
/// The local update every item of one dispatch goes through.
type Apply<T> = Rc<dyn Fn(T) -> Outcome>;

thread_local! {
    /// The local updates of the open dispatches started on this thread, by identity, so a
    /// request made on that thread applies at once.
    static OWNERS: RefCell<HashMap<u64, Rc<dyn Any>>> = RefCell::default();
}

/// The identity the next dispatch registers under.
static NEXT: AtomicU64 = AtomicU64::new(0);

/// Admission and the first failure, shared by every producer of one dispatch.
#[derive(Default)]
struct State {
    /// Whether the dispatch admits no further work.
    closed: bool,
    /// The first failure a local update reported.
    failure: Option<DiagnosticErrorInfo>,
}

impl State {
    /// The first failure, if any.
    fn outcome(&self) -> Outcome {
        self.failure.clone().map_or(Ok(()), Err)
    }

    /// Whether new work is admitted: the first failure, or a refusal once closed.
    fn admitted(&self) -> Outcome {
        self.outcome()?;
        if self.closed {
            return Err(DiagnosticErrorInfo::error("Local dispatch is closed"));
        }
        Ok(())
    }

    /// Keeps the first failure and closes admission.
    fn fail(&mut self, error: &DiagnosticErrorInfo) {
        self.failure.get_or_insert_with(|| error.clone());
        self.closed = true;
    }
}

/// Locks the shared state; a poisoned lock still holds a consistent admission record.
fn lock(state: &Mutex<State>) -> MutexGuard<'_, State> {
    state.lock().unwrap_or_else(PoisonError::into_inner)
}

/// One admitted item and whether its producer waits for the update.
enum Work<T> {
    /// Applied in admission order; the producer does not wait.
    Queued(T),
    /// Applied for a producer on another thread, which waits for the outcome.
    Call(T, sync::SyncSender<Outcome>),
}

/// A sendable producer that can ask for one item to be applied and learn its outcome.
///
/// Cloning keeps the same dispatch. Holding a token neither keeps the dispatch open nor
/// delays [`LocalDispatch::settle`].
pub(crate) struct DispatchToken<T> {
    /// The dispatch's identity on its owning thread.
    id: u64,
    /// The queue the local receiver drains.
    sender: mpsc::UnboundedSender<Work<T>>,
    /// Admission and the first failure.
    state: Arc<Mutex<State>>,
}

impl<T> Clone for DispatchToken<T> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            sender: self.sender.clone(),
            state: Arc::clone(&self.state),
        }
    }
}

impl<T: Send + 'static> DispatchToken<T> {
    /// Queues `work` while the dispatch admits it.
    fn admit(&self, work: Work<T>) -> Outcome {
        let state = lock(&self.state);
        state.admitted()?;
        // The receiver closes only after admission closed, so an admitted item is queued.
        drop(self.sender.send(work));
        Ok(())
    }

    /// Applies `item` and returns the update's outcome before returning.
    ///
    /// On the owning thread the update runs at once, with no lookup borrow held, so it may
    /// request again; on any other thread the item is queued and this call waits until the
    /// owning thread applied it. A closed dispatch refuses the item, and a failed one
    /// returns its first failure.
    pub(crate) fn call(&self, item: T) -> Outcome {
        lock(&self.state).admitted()?;
        let local = OWNERS.with(|owners| owners.borrow().get(&self.id).cloned());
        if let Some(apply) = local.and_then(|apply| apply.downcast::<Apply<T>>().ok()) {
            let result = apply(item);
            if let Err(error) = &result {
                lock(&self.state).fail(error);
            }
            return result;
        }
        let (reply, outcome) = sync::sync_channel(1);
        self.admit(Work::Call(item, reply))?;
        outcome.recv().unwrap_or_else(|_| {
            lock(&self.state).outcome()?;
            Err(DiagnosticErrorInfo::error("Local dispatch is closed"))
        })
    }
}

/// The owning thread's registration of one dispatch; dropping it closes admission.
struct Registration {
    /// The registered identity.
    id: u64,
    /// Keeps the registration on the thread whose lookup holds it.
    local: PhantomData<Rc<()>>,
    /// Admission and the first failure.
    state: Arc<Mutex<State>>,
}

impl Drop for Registration {
    fn drop(&mut self) {
        lock(&self.state).closed = true;
        OWNERS.with(|owners| owners.borrow_mut().remove(&self.id));
    }
}

/// The local delivery of one producer's work.
///
/// A producer that must be sendable hands typed items over through
/// [`LocalDispatch::sender`], which queues them, or [`LocalDispatch::requester`], whose
/// [`DispatchToken::call`] returns the update's outcome. One receiver runs on the terminal
/// runtime and applies each queued item to the local owners. Nothing in the type names
/// what the items are, so every caller shares this one delivery.
///
/// [`LocalDispatch::settle`] closes admission whatever producers are still held and waits
/// until every admitted item was applied. An update that fails ends the delivery, and the
/// first failure is returned by `settle`. Dropping the dispatch closes admission too.
pub(crate) struct LocalDispatch<T> {
    /// The owning thread's registration, released first.
    registration: Registration,
    /// The dispatch's own producer.
    token: DispatchToken<T>,
    /// Tells the receiver to stop waiting for further work.
    close: oneshot::Sender<()>,
    /// Reports that the receiver applied every admitted item.
    delivered: oneshot::Receiver<()>,
}

/// Starts the delivery of `apply` for every item the returned dispatch receives.
pub(crate) fn local_dispatch<T: Send + 'static>(
    runtime: &Rc<dyn TuiRuntime>,
    apply: impl Fn(T) -> Outcome + 'static,
) -> LocalDispatch<T> {
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    let apply: Apply<T> = Rc::new(apply);
    let registered: Rc<dyn Any> = Rc::new(Rc::clone(&apply));
    OWNERS.with(|owners| owners.borrow_mut().insert(id, registered));
    let (sender, mut receiver) = mpsc::unbounded_channel();
    let (close, mut closing) = oneshot::channel::<()>();
    let (drained, delivered) = oneshot::channel::<()>();
    let state: Arc<Mutex<State>> = Arc::default();
    let reported = Arc::clone(&state);
    runtime.spawn_local(Box::pin(async move {
        loop {
            let work = tokio::select! {
                biased;
                _ = &mut closing => break,
                work = receiver.recv() => work,
            };
            let Some(work) = work else { break };
            if !deliver(work, &apply, &reported) {
                break;
            }
        }
        receiver.close();
        while let Ok(work) = receiver.try_recv() {
            if !deliver(work, &apply, &reported) {
                break;
            }
        }
        drop(drained);
        Ok(())
    }));
    LocalDispatch {
        registration: Registration {
            id,
            local: PhantomData,
            state: Arc::clone(&state),
        },
        token: DispatchToken { id, sender, state },
        close,
        delivered,
    }
}

/// Applies one admitted item unless an earlier one failed, keeping a failure for the
/// caller, answering a waiting producer and saying whether the delivery goes on.
fn deliver<T>(work: Work<T>, apply: &Apply<T>, state: &Mutex<State>) -> bool {
    if lock(state).failure.is_some() {
        return false;
    }
    let (item, reply) = match work {
        Work::Queued(item) => (item, None),
        Work::Call(item, reply) => (item, Some(reply)),
    };
    let result = apply(item);
    if let Err(error) = &result {
        lock(state).fail(error);
    }
    let applied = result.is_ok();
    if let Some(reply) = reply {
        // A producer that stopped waiting no longer wants the outcome.
        drop(reply.send(result));
    }
    applied
}

impl<T: Send + 'static> LocalDispatch<T> {
    /// A sendable producer of further items; once the dispatch is closed it sends nothing.
    pub(crate) fn sender(&self) -> Box<dyn FnMut(T) + Send> {
        let token = self.token.clone();
        // A closed dispatch means the local owners are gone; the item has nowhere to go.
        Box::new(move |item| drop(token.admit(Work::Queued(item))))
    }

    /// A sendable producer that learns the outcome of each item it asks for.
    pub(crate) fn requester(&self) -> DispatchToken<T> {
        self.token.clone()
    }

    /// Closes admission, waits until the receiver applied every admitted item, and returns
    /// the first failure of the local update, if any.
    pub(crate) async fn settle(self) -> Outcome {
        let Self {
            registration,
            token,
            close,
            delivered,
        } = self;
        drop(registration);
        // A receiver that already ended needs no signal.
        let _ = close.send(());
        drop(delivered.await);
        lock(&token.state).failure.take().map_or(Ok(()), Err)
    }
}

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use super::{DispatchToken, local_dispatch};
    use crate::components::session_selector::list::tests::session_picker::manual_runtime::ManualRuntime;
    use maestro_app::DiagnosticErrorInfo;
    use std::{
        cell::{Cell, RefCell},
        future::Future,
        rc::Rc,
    };

    /// A failure whose name, message and code must all come back unchanged.
    fn refusal() -> DiagnosticErrorInfo {
        serde_json::from_value(serde_json::json!({
            "name": "PresentationError",
            "message": "the dialog is gone",
            "code": "E_SHOWN"
        }))
        .unwrap()
    }

    /// A dispatch that records every applied item and refuses `refused`.
    fn recording(
        runtime: &ManualRuntime,
        refused: u32,
    ) -> (super::LocalDispatch<u32>, Rc<RefCell<Vec<u32>>>) {
        let applied: Rc<RefCell<Vec<u32>>> = Rc::default();
        let seen = Rc::clone(&applied);
        let dispatch = local_dispatch(&runtime.handle(), move |item: u32| {
            if item == refused {
                return Err(refusal());
            }
            seen.borrow_mut().push(item);
            Ok(())
        });
        (dispatch, applied)
    }

    /// Runs `body` inside a current-thread runtime and task set, where the delivery's
    /// receiver task makes progress.
    fn on_tasks<T>(body: impl Future<Output = T>) -> T {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        tokio::task::LocalSet::new().block_on(&runtime, body)
    }

    /// Runs the delivery's receiver task beside `body`.
    async fn driving<T>(runtime: &ManualRuntime, body: impl Future<Output = T>) -> T {
        let tasks: Vec<_> = runtime
            .take_futures()
            .into_iter()
            .map(|future| tokio::task::spawn_local(async move { drop(future.await) }))
            .collect();
        let value = body.await;
        for task in tasks {
            task.await.unwrap();
        }
        value
    }

    #[test]
    fn presentation_failure_surfaces_at_settle() {
        let runtime = ManualRuntime::new();
        let dispatch = local_dispatch(&runtime.handle(), |item: u32| {
            if item == 2 {
                return Err(DiagnosticErrorInfo::error("the dialog could not be shown"));
            }
            Ok(())
        });
        let mut send = dispatch.sender();
        send(1);
        send(2);
        send(3);
        drop(send);
        let settle = Box::pin(dispatch.settle());
        let error =
            on_tasks(driving(&runtime, settle)).expect_err("the update failure is returned");
        assert_eq!(error.message, "the dialog could not be shown");
    }

    #[test]
    fn retained_requester_does_not_delay_close() {
        let runtime = ManualRuntime::new();
        let (dispatch, applied) = recording(&runtime, u32::MAX);
        let mut send = dispatch.sender();
        let requester = dispatch.requester();
        send(1);
        send(2);
        let settle = Box::pin(dispatch.settle());
        let outcome = on_tasks(driving(&runtime, settle));
        assert!(
            outcome.is_ok(),
            "explicit close must not wait for retained sender"
        );
        assert_eq!(
            applied.borrow().clone(),
            [1, 2],
            "admitted work is applied in order"
        );
        send(3);
        let late = requester.call(4).expect_err("a late call fails");
        assert_eq!(late.message, "Local dispatch is closed");
        assert_eq!(applied.borrow().clone(), [1, 2], "late work paints nothing");
    }

    #[test]
    fn owner_thread_requests_return_original_failure() {
        let runtime = ManualRuntime::new();
        let (dispatch, applied) = recording(&runtime, 2);
        let requester = dispatch.requester();
        assert_eq!(requester.call(1), Ok(()), "a direct call applies at once");
        assert_eq!(applied.borrow().clone(), [1]);
        assert_eq!(
            requester.call(2),
            Err(refusal()),
            "the exact failure returns"
        );
        let mut send = dispatch.sender();
        send(3);
        assert_eq!(
            requester.call(4),
            Err(refusal()),
            "later calls see the failure"
        );
        let settle = Box::pin(dispatch.settle());
        let outcome = on_tasks(driving(&runtime, settle));
        assert_eq!(outcome, Err(refusal()), "settle keeps the same failure");
        assert_eq!(applied.borrow().clone(), [1], "no later work is applied");
    }

    #[test]
    fn foreign_thread_requests_are_acknowledged_before_return() {
        let runtime = ManualRuntime::new();
        let (direct, applied) = recording(&runtime, u32::MAX);
        let requester = direct.requester();
        let outcome = on_tasks(async move {
            tokio::task::spawn_local(async move { requester.call(8) })
                .await
                .unwrap()
        });
        assert_eq!(
            outcome,
            Ok(()),
            "a task on the owning executor calls directly"
        );
        assert_eq!(
            applied.borrow().clone(),
            [8],
            "applied without the receiver task"
        );
        drop(direct);
        let runtime = ManualRuntime::new();
        let (dispatch, applied) = recording(&runtime, 6);
        let requester = dispatch.requester();
        let (answered, answer) = tokio::sync::oneshot::channel();
        let foreign = std::thread::spawn(move || {
            let first = requester.call(5);
            let second = requester.call(6);
            drop(answered.send((first, second)));
        });
        let local = dispatch.requester();
        let (outcomes, spawned) = on_tasks(driving(&runtime, async {
            let outcomes = answer.await.unwrap();
            let seen_then = applied.borrow().clone();
            let spawned = tokio::task::spawn_local(async move { local.call(7) });
            ((outcomes, seen_then), spawned.await.unwrap())
        }));
        foreign.join().unwrap();
        assert_eq!(
            outcomes.0,
            (Ok(()), Err(refusal())),
            "each call waits for its outcome"
        );
        assert_eq!(
            outcomes.1,
            [5],
            "the success was applied before the call returned"
        );
        assert_eq!(
            spawned,
            Err(refusal()),
            "a local task meets the same failure directly"
        );
        let settle = Box::pin(dispatch.settle());
        let outcome = on_tasks(driving(&runtime, settle));
        assert_eq!(outcome, Err(refusal()));
    }

    #[test]
    fn registration_drop_closes_unfinished_delivery() {
        let runtime = ManualRuntime::new();
        let observed = Rc::new(Cell::new(0));
        let owned = Rc::clone(&observed);
        let dispatch = local_dispatch(&runtime.handle(), move |item: u32| {
            owned.set(owned.get() + item);
            Ok(())
        });
        let requester = dispatch.requester();
        let mut send = dispatch.sender();
        send(1);
        drop(dispatch);
        let refused = requester
            .call(2)
            .expect_err("a dropped dispatch admits nothing");
        assert_eq!(refused.message, "Local dispatch is closed");
        assert_eq!(
            Rc::strong_count(&observed),
            2,
            "the receiver still owns the update"
        );
        on_tasks(driving(&runtime, async {}));
        assert_eq!(observed.get(), 1, "the admitted item was drained");
        assert_eq!(
            Rc::strong_count(&observed),
            1,
            "the receiver released the update"
        );
        send(3);
        assert_eq!(observed.get(), 1, "a late send applies nothing");
    }

    #[test]
    fn synchronous_request_reentry_keeps_instance_identity() {
        let runtime = ManualRuntime::new();
        let slot: Rc<RefCell<Option<DispatchToken<u32>>>> = Rc::default();
        let applied: Rc<RefCell<Vec<u32>>> = Rc::default();
        let (inner, seen) = (Rc::clone(&slot), Rc::clone(&applied));
        let first = local_dispatch(&runtime.handle(), move |item: u32| {
            seen.borrow_mut().push(item);
            let again = inner.borrow().clone();
            match again {
                Some(token) if item == 0 => token.call(1),
                _ => Ok(()),
            }
        });
        let held = Rc::downgrade(&applied);
        drop(applied);
        *slot.borrow_mut() = Some(first.requester());
        let (second, _) = recording(&runtime, 9);
        assert_eq!(second.requester().call(9), Err(refusal()));
        assert_eq!(
            first.requester().call(0),
            Ok(()),
            "reentry needs no lookup borrow"
        );
        assert_eq!(held.upgrade().unwrap().borrow().clone(), [0, 1]);
        let settling = Box::pin(async { (first.settle().await, second.settle().await) });
        let outcomes = on_tasks(driving(&runtime, settling));
        assert_eq!(
            outcomes,
            (Ok(()), Err(refusal())),
            "each instance keeps its own outcome"
        );
        drop(slot);
        assert!(
            held.upgrade().is_none(),
            "no registration keeps the update after close"
        );
    }

    #[test]
    fn clean_delivery_settles_without_a_failure() {
        let runtime = ManualRuntime::new();
        let applied: Rc<RefCell<Vec<u32>>> = Rc::default();
        let seen = Rc::clone(&applied);
        let dispatch = local_dispatch(&runtime.handle(), move |item: u32| {
            seen.borrow_mut().push(item);
            Ok(())
        });
        let mut send = dispatch.sender();
        send(7);
        drop(send);
        let settle = Box::pin(dispatch.settle());
        let outcome = on_tasks(driving(&runtime, settle));
        assert!(outcome.is_ok());
        assert_eq!(applied.borrow().clone(), [7]);
    }
}
